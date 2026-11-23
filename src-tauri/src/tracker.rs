//! Play / Pausa / Reanudar / Stop.
//!
//! Cada acción es una transacción local (SQLite) que, además de mover el
//! cronómetro, encola las escrituras equivalentes en ClickUp. Así la acción es
//! instantánea para el usuario y nunca se pierde aunque no haya red.

use chrono::{Local, TimeZone};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::clickup::{norm, Task};
use crate::config::Settings;
use crate::db::{self, NewSession};
use crate::error::{AppError, AppResult};
use crate::state::{auto_filled, bucket, fresh_task, missing_fields, now_ms, AppState, Bucket};

/// Escritura pendiente hacia ClickUp. Los campos se referencian por nombre y se
/// resuelven a su id al ejecutar (con las definiciones de la lista del ticket).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Op {
    SetDate { field: String, ms: i64 },
    SetDropdown { field: String, option: String },
    /// Agrega una línea al final del texto que ya tenga el campo.
    AppendText { field: String, line: String },
    Status { status: String },
    Comment { text: String },
    TimeEntry { start: i64, duration: i64 },
    /// Valor ya en el formato de la API (lo arma el formulario de campos).
    SetRaw { field_id: String, name: String, value: serde_json::Value, time: bool },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Job {
    pub list_id: String,
    #[serde(flatten)]
    pub op: Op,
}

fn label_of(task: &Task, op: &Op) -> String {
    let what = match op {
        Op::SetDate { field, .. } | Op::SetDropdown { field, .. } | Op::AppendText { field, .. } => field.clone(),
        Op::Status { status } => format!("Estado → {status}"),
        Op::Comment { .. } => "Comentario".into(),
        Op::TimeEntry { .. } => "Registro de tiempo".into(),
        Op::SetRaw { name, .. } => name.clone(),
    };
    format!("{} · {what}", task.display_id())
}

fn push(c: &Connection, task: &Task, op: Op, now: i64) -> rusqlite::Result<()> {
    let label = label_of(task, &op);
    let job = Job { list_id: task.list.id.clone(), op };
    let json = serde_json::to_string(&job).expect("Job siempre serializa");
    db::enqueue(c, &task.id, &label, &json, now)
}

/// Encola varias operaciones de un ticket en una sola transacción.
pub fn enqueue_all(state: &AppState, task: &Task, ops: Vec<Op>) -> AppResult<()> {
    let now = now_ms();
    state.db.tx(|c| {
        for op in ops {
            push(c, task, op, now)?;
        }
        Ok(())
    })
}

pub fn fmt_local(ms: i64) -> String {
    Local
        .timestamp_millis_opt(ms)
        .single()
        .map(|d| d.format("%d/%m/%Y %H:%M").to_string())
        .unwrap_or_default()
}

pub fn fmt_duration(ms: i64) -> String {
    let mins = (ms.max(0) + 30_000) / 60_000;
    format!("{}h {:02}m", mins / 60, mins % 60)
}

/// Tramo trabajado → registro de tiempo nativo de ClickUp (si dura al menos 1 min).
fn push_segment(c: &Connection, task: &Task, seg: Option<(i64, i64)>, now: i64) -> rusqlite::Result<()> {
    if let Some((start, end)) = seg {
        if end - start >= 60_000 {
            push(c, task, Op::TimeEntry { start, duration: end - start }, now)?;
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------

/// Busca el ticket en caché o, si no está (p. ej. recién asignado), en la API.
async fn load_task(state: &AppState, task_id: &str) -> AppResult<Task> {
    if let Some(t) = state.cached_task(task_id).await {
        return Ok(t);
    }
    let t = state.client().await?.task(task_id).await?;
    Ok(t)
}

pub async fn play(state: &AppState, task_id: &str) -> AppResult<()> {
    let s = state.settings();
    let task = load_task(state, task_id).await?;

    // Si otro ticket está corriendo, se pausa automáticamente (un solo cronómetro activo).
    if let Some(run) = state.db.running_session()? {
        if run.task_id == task_id {
            return Ok(());
        }
        let other = load_task(state, &run.task_id).await?;
        pause(state, &other, &format!("Cambio al ticket {}", task.display_id()), &s)?;
    }

    let now = now_ms();
    if let Some(sess) = state.db.open_session_for_task(task_id)? {
        // Estaba en pausa → reanudar.
        return state.db.tx(|c| {
            resume_in_tx(c, &task, sess.id, now, &s)?;
            db::open_segment(c, sess.id, &task.id, now)?;
            db::set_session_state(c, sess.id, "running", now)
        });
    }

    let b = bucket(&task, &s);
    state.db.tx(|c| {
        let id = db::insert_session(
            c,
            &NewSession {
                task_id: &task.id,
                task_label: task.display_id(),
                task_name: &task.name,
                task_url: &task.url,
                list_id: &task.list.id,
            },
            now,
        )?;
        db::open_segment(c, id, &task.id, now)?;

        // No se sobrescribe una fecha de inicio que ya exista (p. ej. ticket reabierto).
        if !task.field_is_set(&s.fields.start_date) {
            push(c, &task, Op::SetDate { field: s.fields.start_date.clone(), ms: now }, now)?;
        }
        if matches!(b, Bucket::NotStarted | Bucket::Reopened | Bucket::Done)
            && norm(&task.status.status) != norm(&s.statuses.in_progress)
        {
            push(c, &task, Op::Status { status: s.statuses.in_progress.clone() }, now)?;
        }
        if !task.field_is_set(&s.fields.pause_state) {
            push(c, &task, Op::SetDropdown { field: s.fields.pause_state.clone(), option: s.fields.pause_state_none.clone() }, now)?;
        }
        if b == Bucket::Reopened && !task.field_is_set(&s.fields.reopened_date) {
            push(c, &task, Op::SetDate { field: s.fields.reopened_date.clone(), ms: now }, now)?;
        }
        Ok(())
    })
}

/// Pausa el ticket que está corriendo.
pub async fn pause_current(state: &AppState, reason: &str) -> AppResult<()> {
    let s = state.settings();
    let run = state
        .db
        .running_session()?
        .ok_or_else(|| AppError::invalid("No hay ningún ticket corriendo."))?;
    let task = load_task(state, &run.task_id).await?;
    pause(state, &task, reason, &s)
}

fn pause(state: &AppState, task: &Task, reason: &str, s: &Settings) -> AppResult<()> {
    let sess = state
        .db
        .open_session_for_task(&task.id)?
        .filter(|x| x.state == "running")
        .ok_or_else(|| AppError::invalid("El ticket no está corriendo."))?;

    // Primer espacio "Inicio Pausa N" libre, tanto en local como en ClickUp
    // (alguien pudo llenarlo a mano). Si no queda ninguno, va como comentario.
    let used = state.db.used_pause_slots(&task.id)?;
    let slot = (1..=s.fields.pause_starts.len() as i64)
        .find(|n| {
            let i = (*n - 1) as usize;
            !used.contains(n)
                && !task.field_is_set(&s.fields.pause_starts[i])
                && !task.field_is_set(s.fields.pause_ends.get(i).map(String::as_str).unwrap_or_default())
        })
        .unwrap_or(0);
    let seq = state.db.pause_count(&task.id)? + 1;
    let reason = reason.trim();
    let now = now_ms();

    state.db.tx(|c| {
        let seg = db::close_segment(c, sess.id, now)?;
        push_segment(c, task, seg, now)?;
        db::insert_pause(c, sess.id, &task.id, slot, seq, now, reason)?;
        db::set_session_state(c, sess.id, "paused", now)?;

        if slot > 0 {
            let field = s.fields.pause_starts[(slot - 1) as usize].clone();
            push(c, task, Op::SetDate { field, ms: now }, now)?;
        } else {
            let mut text = format!("⏸ Pausa #{seq} iniciada el {}.", fmt_local(now));
            if !reason.is_empty() {
                text.push_str(&format!("\nMotivo: {reason}"));
            }
            push(c, task, Op::Comment { text }, now)?;
        }
        push(c, task, Op::SetDropdown { field: s.fields.pause_state.clone(), option: s.fields.pause_state_paused.clone() }, now)?;
        if !reason.is_empty() {
            let line = format!("[Pausa {seq} · {}] {reason}", fmt_local(now));
            push(c, task, Op::AppendText { field: s.fields.pause_reason.clone(), line }, now)?;
        }
        Ok(())
    })
}

/// Hora de salida (ms) a partir de la cual se pausa por fin de turno el tramo que
/// empezó en `seg_start`. `None` si ese tramo empezó en un día libre o después de la
/// salida: es tiempo extra a propósito y no se interrumpe.
pub fn shift_end_for(s: &Settings, seg_start: i64) -> Option<i64> {
    use chrono::Timelike;
    let start = Local.timestamp_millis_opt(seg_start).single()?;
    let (_, end) = s.shift_for(start.date_naive())?;
    if start.hour() * 60 + start.minute() >= end {
        return None;
    }
    let at = start.date_naive().and_hms_opt(end / 60, end % 60, 0)?;
    Local.from_local_datetime(&at).earliest().map(|d| d.timestamp_millis())
}

/// Si el ticket que corre ya pasó la hora de salida, lo pausa solo en la app con
/// la hora exacta de salida (aunque la app estuviera cerrada o el equipo suspendido).
/// Solo se envía a ClickUp el registro de tiempo del tramo trabajado; ni campos de
/// pausa, ni estado de pausa, ni motivo, ni comentario. Devuelve el ticket pausado.
pub async fn pause_for_shift_end(state: &AppState) -> AppResult<Option<Task>> {
    let s = state.settings();
    if !s.auto_pause_shift {
        return Ok(None);
    }
    let Some(run) = state.db.running_session()? else { return Ok(None) };
    let Some(seg_start) = state.db.open_segment_start(run.id)? else { return Ok(None) };
    let Some(end) = shift_end_for(&s, seg_start) else { return Ok(None) };
    let now = now_ms();
    if now < end {
        return Ok(None);
    }
    let task = load_task(state, &run.task_id).await?;
    state.db.tx(|c| {
        let seg = db::close_segment(c, run.id, end)?;
        push_segment(c, &task, seg, now)?;
        db::insert_pause(c, run.id, &task.id, -1, 0, end, "Fin de turno")?;
        db::set_session_state(c, run.id, "paused", now)
    })?;
    log::info!("fin de turno: {} pausado a las {}", task.display_id(), fmt_local(end));
    Ok(Some(task))
}

/// Cierra la pausa abierta de una sesión y encola su fecha de fin.
fn resume_in_tx(c: &Connection, task: &Task, session_id: i64, now: i64, s: &Settings) -> rusqlite::Result<()> {
    if let Some(p) = db::close_pause(c, session_id, now)? {
        if p.slot < 0 {
            // Fin de turno: la pausa nunca se registró en ClickUp, así que reanudar tampoco.
            return Ok(());
        }
        if p.slot > 0 {
            if let Some(field) = s.fields.pause_ends.get((p.slot - 1) as usize) {
                push(c, task, Op::SetDate { field: field.clone(), ms: now }, now)?;
            }
        } else {
            let text = format!(
                "▶ Pausa #{} finalizada el {} (duró {}).",
                p.seq,
                fmt_local(now),
                fmt_duration(now - p.started_at)
            );
            push(c, task, Op::Comment { text }, now)?;
        }
        push(c, task, Op::SetDropdown { field: s.fields.pause_state.clone(), option: s.fields.pause_state_closed.clone() }, now)?;
    }
    Ok(())
}

/// Finaliza el ticket. Es obligatorio que estén llenos los campos obligatorios que
/// le aplican al usuario: si falta alguno, devuelve el error con la lista y no finaliza.
pub async fn stop(state: &AppState, task_id: &str) -> AppResult<()> {
    let s = state.settings();
    let task = fresh_task(state, task_id).await?;
    let sess = state
        .db
        .open_session_for_task(task_id)?
        .ok_or_else(|| AppError::invalid("El ticket no tiene un cronómetro activo."))?;

    let defs = state.list_fields(&task.list.id).await.ok();
    let missing = missing_fields(&task, defs.as_ref(), &s, &auto_filled(&s));
    if !missing.is_empty() {
        return Err(AppError::MissingFields(missing));
    }

    let now = now_ms();
    let pauses = state.db.pause_count(task_id)?;
    state.db.tx(|c| {
        if sess.state == "paused" {
            resume_in_tx(c, &task, sess.id, now, &s)?;
        } else {
            let seg = db::close_segment(c, sess.id, now)?;
            push_segment(c, &task, seg, now)?;
        }
        db::set_session_state(c, sess.id, "stopped", now)?;
        push(c, &task, Op::SetDate { field: s.fields.end_date.clone(), ms: now }, now)?;
        push(c, &task, Op::Status { status: s.statuses.done.clone() }, now)?;
        Ok(())
    })?;

    let worked = state.db.task_worked_ms(task_id, now)?;
    let text = format!(
        "⏹ Ticket finalizado el {} con TicketPilot.\nTiempo trabajado: {} · Pausas: {pauses}",
        fmt_local(now),
        fmt_duration(worked)
    );
    state.db.tx(|c| push(c, &task, Op::Comment { text }, now))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(y: i32, mo: u32, d: u32, h: u32, mi: u32) -> i64 {
        let at = chrono::NaiveDate::from_ymd_opt(y, mo, d).unwrap().and_hms_opt(h, mi, 0).unwrap();
        Local.from_local_datetime(&at).earliest().unwrap().timestamp_millis()
    }

    #[test]
    fn fin_de_turno() {
        let s = Settings::default();
        // Jueves: empezó 9:00 → se pausa a las 17:00.
        assert_eq!(shift_end_for(&s, ms(2026, 11, 26, 9, 0)), Some(ms(2026, 11, 26, 17, 0)));
        // Viernes: salida 16:30.
        assert_eq!(shift_end_for(&s, ms(2026, 11, 27, 6, 45)), Some(ms(2026, 11, 27, 16, 30)));
        // Reanudó después de la salida (tiempo extra) o en sábado: no se interrumpe.
        assert_eq!(shift_end_for(&s, ms(2026, 11, 26, 17, 10)), None);
        assert_eq!(shift_end_for(&s, ms(2026, 11, 28, 10, 0)), None);
    }
}
