//! Reportes de tiempo trabajado y tickets ejecutados por día, semana o mes.
//! Se consulta a ClickUp en cada apertura: no se guarda nada en la base local.

use std::collections::{HashMap, HashSet};

use chrono::{Datelike, Duration, Local, NaiveDate, TimeZone};
use serde::Serialize;
use serde_json::Value;
use tauri::State;

use crate::clickup::TaskQuery;
use crate::error::{AppError, AppResult};
use crate::state::{now_ms, AppState};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportTicket {
    pub task_id: String,
    pub label: String,
    pub name: String,
    pub url: String,
    pub worked_ms: i64,
    /// Horas estimadas en ClickUp (ms), si están llenas.
    pub estimate_ms: Option<i64>,
    /// Si se finalizó dentro del período, la hora.
    pub finished_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportDay {
    /// Inicio del día (ms, hora local).
    pub at: i64,
    pub worked_ms: i64,
    pub tickets: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub period: String,
    pub from: i64,
    /// Fin exclusivo del período.
    pub to: i64,
    pub worked_ms: i64,
    /// Tickets con tiempo registrado en el período.
    pub worked_tickets: usize,
    /// Tickets finalizados en el período.
    pub finished_tickets: usize,
    pub days: Vec<ReportDay>,
    pub tickets: Vec<ReportTicket>,
}

fn day_ms(d: NaiveDate) -> i64 {
    d.and_hms_opt(0, 0, 0)
        .and_then(|x| Local.from_local_datetime(&x).earliest())
        .map(|x| x.timestamp_millis())
        .unwrap_or(0)
}

/// Rango [desde, hasta) del período que contiene `today` desplazado `offset` períodos.
/// La semana va de lunes a domingo.
pub fn range(period: &str, offset: i64, today: NaiveDate) -> AppResult<(NaiveDate, NaiveDate)> {
    Ok(match period {
        "day" => {
            let d = today + Duration::days(offset);
            (d, d + Duration::days(1))
        }
        "week" => {
            let monday = today - Duration::days(today.weekday().num_days_from_monday() as i64) + Duration::weeks(offset);
            (monday, monday + Duration::weeks(1))
        }
        "month" => {
            let total = today.year() as i64 * 12 + today.month0() as i64 + offset;
            let (y, m) = ((total.div_euclid(12)) as i32, (total.rem_euclid(12)) as u32 + 1);
            let first = NaiveDate::from_ymd_opt(y, m, 1).ok_or_else(|| AppError::invalid("Mes fuera de rango."))?;
            let next = if m == 12 { NaiveDate::from_ymd_opt(y + 1, 1, 1) } else { NaiveDate::from_ymd_opt(y, m + 1, 1) };
            (first, next.ok_or_else(|| AppError::invalid("Mes fuera de rango."))?)
        }
        _ => return Err(AppError::invalid("Período desconocido.")),
    })
}

/// Número que puede venir como texto o como número.
fn num(v: Option<&Value>) -> Option<i64> {
    match v? {
        Value::Number(n) => n.as_i64(),
        Value::String(s) => s.parse().ok(),
        _ => None,
    }
}

fn text(v: Option<&Value>) -> Option<String> {
    v?.as_str().filter(|s| !s.is_empty()).map(str::to_owned)
}

/// Datos que no se guardan: todo se consulta a ClickUp al abrir el reporte.
/// Tiempo = registros de tiempo del usuario; finalizados = tickets asignados con
/// «Fecha Fin» dentro del período.
#[tauri::command]
pub async fn get_report(state: State<'_, AppState>, period: String, offset: i64) -> AppResult<Report> {
    let s = state.settings();
    let team_id = s.team_id.clone().ok_or_else(|| AppError::invalid("Primero conecta ClickUp en Ajustes."))?;
    let client = state.client().await?;
    let cached_user = state.cache.lock().await.user.clone();
    let user = match cached_user {
        Some(u) => u,
        None => client.me().await?,
    };

    let (a, b) = range(&period, offset.min(0), Local::now().date_naive())?;
    let (from, to) = (day_ms(a), day_ms(b));
    let now = now_ms();

    // Los registros que empezaron hasta un día antes pueden cruzar el inicio del período.
    let entries = client.time_entries(&team_id, from - 86_400_000, to - 1).await?;
    // Tramos (task_id, inicio, fin) recortados al período.
    let mut spans: Vec<(String, i64, i64)> = Vec::new();
    let mut meta: HashMap<String, (String, String)> = HashMap::new();
    for e in &entries {
        let Some(task) = e.get("task").filter(|t| t.is_object()) else { continue };
        let Some(id) = text(task.get("id")) else { continue };
        let Some(start) = num(e.get("start")) else { continue };
        let end = match num(e.get("end")) {
            Some(x) if x > 0 => x,
            // Cronómetro de ClickUp corriendo: duración negativa.
            _ => now,
        };
        let (st, en) = (start.max(from), end.min(to).min(now));
        if en <= st {
            continue;
        }
        meta.entry(id.clone()).or_insert_with(|| {
            let label = text(task.get("custom_id")).unwrap_or_else(|| id.clone());
            (label, text(task.get("name")).unwrap_or_default())
        });
        spans.push((id, st, en));
    }

    // Finalizados: solo hace falta mirar los tickets modificados desde el inicio del período.
    let done = client
        .assigned_tasks(&TaskQuery {
            team_id: &team_id,
            user_id: user.id,
            list_ids: &s.list_ids,
            include_closed: true,
            date_done_gt: None,
            date_updated_gt: Some(from),
        })
        .await?;
    let mut estimates: HashMap<String, i64> = HashMap::new();
    for t in &done {
        if let Some(e) = t.estimate_ms(&s.fields.estimate) {
            estimates.insert(t.id.clone(), e);
        }
    }
    let mut finished: HashMap<String, i64> = HashMap::new();
    for t in &done {
        let at = t
            .field_text(&s.fields.end_date)
            .and_then(|v| v.parse::<i64>().ok())
            .or(t.date_done);
        if let Some(at) = at.filter(|at| (from..to).contains(at)) {
            finished.insert(t.id.clone(), at);
            meta.entry(t.id.clone()).or_insert_with(|| (t.display_id().to_owned(), t.name.clone()));
        }
    }

    let mut worked: HashMap<String, i64> = HashMap::new();
    for (id, st, en) in &spans {
        *worked.entry(id.clone()).or_default() += en - st;
    }

    // El registro de tiempo no siempre trae el código (TKM-…): se completa con la caché.
    {
        let cache = state.cache.lock().await;
        for (id, (label, name)) in meta.iter_mut() {
            if let Some(t) = cache.tasks.iter().find(|t| &t.id == id) {
                if let Some(e) = t.estimate_ms(&s.fields.estimate) {
                    estimates.entry(id.clone()).or_insert(e);
                }
                *label = t.display_id().to_owned();
                if name.is_empty() {
                    *name = t.name.clone();
                }
            }
        }
    }

    let mut tickets: Vec<ReportTicket> = meta
        .into_iter()
        .filter(|(id, _)| worked.contains_key(id) || finished.contains_key(id))
        .map(|(id, (label, name))| ReportTicket {
            worked_ms: worked.get(&id).copied().unwrap_or(0),
            finished_at: finished.get(&id).copied(),
            estimate_ms: estimates.get(&id).copied(),
            url: format!("https://app.clickup.com/t/{id}"),
            task_id: id,
            label,
            name,
        })
        .collect();
    tickets.sort_by(|x, y| y.worked_ms.cmp(&x.worked_ms).then(x.label.cmp(&y.label)));

    let mut days = Vec::new();
    let mut d = a;
    while d < b {
        let (ds, de) = (day_ms(d), day_ms(d + Duration::days(1)));
        let mut ids = HashSet::new();
        let mut ms = 0;
        for (id, st, en) in &spans {
            let (x, y) = ((*st).max(ds), (*en).min(de));
            if y > x {
                ms += y - x;
                ids.insert(id);
            }
        }
        days.push(ReportDay { at: ds, worked_ms: ms, tickets: ids.len() });
        d += Duration::days(1);
    }

    Ok(Report {
        period,
        from,
        to,
        worked_ms: worked.values().sum(),
        worked_tickets: worked.len(),
        finished_tickets: finished.len(),
        days,
        tickets,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rangos() {
        let mie = NaiveDate::from_ymd_opt(2026, 11, 25).unwrap();
        let d = |y, m, d| NaiveDate::from_ymd_opt(y, m, d).unwrap();
        assert_eq!(range("day", -1, mie).unwrap(), (d(2026, 11, 24), d(2026, 11, 25)));
        assert_eq!(range("week", 0, mie).unwrap(), (d(2026, 11, 23), d(2026, 11, 30)));
        assert_eq!(range("month", 0, mie).unwrap(), (d(2026, 11, 1), d(2026, 12, 1)));
        assert_eq!(range("month", 1, mie).unwrap(), (d(2026, 12, 1), d(2027, 1, 1)));
        assert_eq!(range("month", -11, mie).unwrap(), (d(2025, 12, 1), d(2026, 1, 1)));
    }
}
