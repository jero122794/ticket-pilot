//! Bucle de fondo: replica el outbox en ClickUp, recarga los tickets,
//! lanza los recordatorios de campos pendientes y el resumen diario por correo.

use std::collections::HashSet;
use std::sync::atomic::Ordering;
use std::time::Duration;

use serde_json::json;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_notification::NotificationExt;

use crate::clickup::{norm, ClickUp, FieldDef, TaskQuery};
use crate::config::parse_hhmm;
use crate::db::OutboxRow;
use crate::error::{AppError, AppResult};
use crate::state::{bucket, broadcast, missing_fields, now_ms, AppState, Bucket};
use crate::tracker::{Job, Op};
use crate::{mail, secrets};

const TICK: Duration = Duration::from_secs(15);
const MAX_ATTEMPTS: i64 = 12;
const RECENT_DONE_DAYS: i64 = 30;

pub const EVENT_REMINDER: &str = "reminder";

pub fn spawn(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut last_poll: i64 = 0;
        let mut last_minute: i64 = 0;
        loop {
            let state = app.state::<AppState>();
            let s = state.settings();
            let now = now_ms();
            let configured = secrets::get(secrets::CLICKUP_TOKEN).ok().flatten().is_some();

            let flushed = if configured { flush_outbox(&state).await } else { 0 };
            let dirty = state.dirty.swap(false, Ordering::SeqCst);
            let poll_due = now - last_poll >= (s.poll_minutes.max(1) as i64) * 60_000;

            if configured && (dirty || poll_due || flushed > 0) {
                broadcast(&app, true).await;
                if let Err(e) = refresh(&state).await {
                    log::warn!("refresh: {e}");
                    if matches!(e, AppError::Unauthorized) {
                        state.reset_client().await;
                    }
                    state.cache.lock().await.last_error = Some(e.to_string());
                }
                last_poll = now_ms();
            }

            if now - last_minute >= 60_000 {
                last_minute = now;
                if configured {
                    remind(&app, &state).await;
                }
                if let Err(e) = mail::tick(&state).await {
                    log::warn!("correo diario: {e}");
                    state.cache.lock().await.last_error = Some(e.to_string());
                }
            }

            broadcast(&app, false).await;

            tokio::select! {
                _ = tokio::time::sleep(TICK) => {}
                _ = state.wake.notified() => {}
            }
        }
    });
}

// ---------------------------------------------------------------------------
// Recarga de tickets
// ---------------------------------------------------------------------------

pub async fn refresh(state: &AppState) -> AppResult<()> {
    let client = state.client().await?;

    // Se copia primero para no retener el candado de la caché durante la petición.
    let cached_user = state.cache.lock().await.user.clone();
    let user = match cached_user {
        Some(u) => u,
        None => client.me().await?,
    };

    let mut s = state.settings();
    let team_id = match s.team_id.clone() {
        Some(t) => t,
        None => {
            let team = client
                .teams()
                .await?
                .into_iter()
                .next()
                .ok_or_else(|| AppError::invalid("El token no tiene acceso a ningún workspace."))?;
            s.team_id = Some(team.id.clone());
            state.save_settings(s.clone())?;
            team.id
        }
    };

    let open = client
        .assigned_tasks(&TaskQuery {
            team_id: &team_id,
            user_id: user.id,
            list_ids: &s.list_ids,
            include_closed: false,
            date_done_gt: None,
        })
        .await?;
    let recent_done = client
        .assigned_tasks(&TaskQuery {
            team_id: &team_id,
            user_id: user.id,
            list_ids: &s.list_ids,
            include_closed: true,
            date_done_gt: Some(now_ms() - RECENT_DONE_DAYS * 86_400_000),
        })
        .await
        .unwrap_or_default();

    let mut seen = HashSet::new();
    let tasks: Vec<_> = open
        .into_iter()
        .chain(recent_done)
        .filter(|t| seen.insert(t.id.clone()))
        .collect();

    // Definiciones de campos de cada lista nueva (una petición por lista, una sola vez).
    let known: HashSet<String> = {
        let c = state.cache.lock().await;
        c.fields.keys().chain(c.no_access.iter()).cloned().collect()
    };
    let lists: HashSet<String> = tasks.iter().map(|t| t.list.id.clone()).filter(|l| !known.contains(l)).collect();
    for list_id in lists {
        match client.list_fields(&list_id).await {
            Ok(f) => {
                state.cache.lock().await.fields.insert(list_id, f);
            }
            Err(AppError::ClickUp { status: 401 | 403, .. }) => {
                log::info!("sin acceso a la lista {list_id}: se usan los datos de cada ticket");
                state.cache.lock().await.no_access.insert(list_id);
            }
            Err(e) => log::warn!("campos de la lista {list_id}: {e}"),
        }
    }

    let mut cache = state.cache.lock().await;
    cache.user = Some(user);
    cache.tasks = tasks;
    cache.last_sync = Some(now_ms());
    cache.last_error = None;
    Ok(())
}

// ---------------------------------------------------------------------------
// Outbox
// ---------------------------------------------------------------------------

/// Ejecuta las escrituras pendientes en orden. Devuelve cuántas se completaron.
async fn flush_outbox(state: &AppState) -> usize {
    let rows = match state.db.outbox_due(now_ms(), 25) {
        Ok(r) => r,
        Err(e) => {
            log::error!("outbox: {e}");
            return 0;
        }
    };
    if rows.is_empty() {
        return 0;
    }
    let client = match state.client().await {
        Ok(c) => c,
        Err(_) => return 0,
    };

    let mut done = 0;
    // Si una operación de un ticket falla, las siguientes del mismo ticket esperan.
    let mut blocked: HashSet<String> = HashSet::new();
    for row in rows {
        if blocked.contains(&row.task_id) {
            continue;
        }
        let result = match serde_json::from_str::<Job>(&row.op) {
            Ok(job) => execute(state, &client, &row.task_id, job).await,
            Err(e) => Err(AppError::from(e)),
        };
        match result {
            Ok(()) => {
                let _ = state.db.outbox_done(row.id);
                done += 1;
            }
            Err(e) => {
                blocked.insert(row.task_id.clone());
                let rate_limited = matches!(e, AppError::RateLimited(_));
                schedule_retry(state, &row, &e);
                if rate_limited || matches!(e, AppError::Http(_)) {
                    break; // sin red o limitado: no tiene sentido seguir intentando ahora
                }
            }
        }
    }
    done
}

fn schedule_retry(state: &AppState, row: &OutboxRow, e: &AppError) {
    let now = now_ms();
    let wait_ms = match e {
        AppError::RateLimited(secs) => (*secs as i64) * 1000,
        // 15 s, 30 s, 1 min, 2 min… hasta 30 min.
        _ => (15_000i64 << row.attempts.min(7)).min(30 * 60_000),
    };
    let permanent = e.is_permanent() || row.attempts + 1 >= MAX_ATTEMPTS;
    if let Err(err) = state.db.outbox_retry_later(row, &e.to_string(), now + wait_ms, permanent) {
        log::error!("outbox retry: {err}");
    }
}

async fn field_def(state: &AppState, client: &ClickUp, task_id: &str, list_id: &str, name: &str) -> AppResult<FieldDef> {
    let find = |defs: &[FieldDef]| defs.iter().find(|d| norm(&d.name) == norm(name)).cloned();
    let defs = match state.list_fields(list_id).await {
        Ok(d) => d,
        // Sin acceso a la lista: el propio ticket trae id, tipo y opciones de sus campos.
        Err(AppError::ClickUp { status: 401 | 403, .. }) => {
            let task = client.task(task_id).await?;
            let defs: Vec<FieldDef> = task.custom_fields.iter().map(|f| f.to_def()).collect();
            return find(&defs)
                .ok_or_else(|| AppError::invalid(format!("El ticket no tiene el campo «{name}».")));
        }
        Err(e) => return Err(e),
    };
    if let Some(d) = find(&defs) {
        return Ok(d);
    }
    // Puede que el campo se haya creado después: se recargan las definiciones una vez.
    state.cache.lock().await.fields.remove(list_id);
    find(&state.list_fields(list_id).await?)
        .ok_or_else(|| AppError::invalid(format!("La lista no tiene el campo «{name}». Revisa los nombres en Ajustes.")))
}

async fn execute(state: &AppState, client: &ClickUp, task_id: &str, job: Job) -> AppResult<()> {
    match job.op {
        Op::SetDate { field, ms } => {
            let def = field_def(state, client, task_id, &job.list_id, &field).await?;
            client.set_field(task_id, &def.id, &json!(ms), true).await
        }
        Op::SetDropdown { field, option } => {
            let def = field_def(state, client, task_id, &job.list_id, &field).await?;
            let opt = def
                .option_id(&option)
                .ok_or_else(|| AppError::invalid(format!("El campo «{field}» no tiene la opción «{option}».")))?;
            client.set_field(task_id, &def.id, &json!(opt), false).await
        }
        Op::AppendText { field, line } => {
            let def = field_def(state, client, task_id, &job.list_id, &field).await?;
            let current = client.task(task_id).await?.field_text(&field).unwrap_or_default();
            let text = if current.trim().is_empty() {
                line
            } else {
                format!("{}\n{line}", current.trim_end())
            };
            client.set_field(task_id, &def.id, &json!(text), false).await
        }
        Op::SetRaw { field_id, value, time, .. } => client.set_field(task_id, &field_id, &value, time).await,
        Op::Status { status } => client.set_status(task_id, &status).await,
        Op::Comment { text } => client.comment(task_id, &text).await,
        Op::TimeEntry { start, duration } => {
            let team = state
                .settings()
                .team_id
                .ok_or_else(|| AppError::invalid("Aún no se conoce el workspace de ClickUp."))?;
            client.time_entry(&team, task_id, start, duration).await
        }
    }
}

// ---------------------------------------------------------------------------
// Recordatorios de campos sin llenar
// ---------------------------------------------------------------------------

fn within_work_hours(start: &str, end: &str) -> bool {
    use chrono::{Datelike, Local, Timelike, Weekday};
    let now = Local::now();
    if matches!(now.weekday(), Weekday::Sat | Weekday::Sun) {
        return false;
    }
    let m = now.hour() * 60 + now.minute();
    match (parse_hhmm(start), parse_hhmm(end)) {
        (Some(a), Some(b)) => a <= m && m < b,
        _ => true,
    }
}

async fn remind(app: &AppHandle, state: &AppState) {
    let s = state.settings();
    if s.reminder_minutes == 0 || !within_work_hours(&s.work_start, &s.work_end) {
        return;
    }
    let now = now_ms();
    let every = s.reminder_minutes as i64 * 60_000;
    let active: HashSet<String> = state
        .db
        .current_session()
        .ok()
        .flatten()
        .map(|x| x.task_id)
        .into_iter()
        .collect();

    let mut lines = Vec::new();
    let mut ids = Vec::new();
    {
        let mut cache = state.cache.lock().await;
        let pending: Vec<(String, String, Vec<String>)> = cache
            .tasks
            .iter()
            .filter(|t| {
                active.contains(&t.id) || matches!(bucket(t, &s), Bucket::InProgress | Bucket::Reopened)
            })
            .filter_map(|t| {
                let missing = missing_fields(t, cache.fields.get(&t.list.id), &s, &[s.fields.end_date.as_str()]);
                (!missing.is_empty()).then(|| (t.id.clone(), t.display_id().to_owned(), missing))
            })
            .collect();
        for (id, label, missing) in pending {
            let last = cache.reminded.get(&id).copied().unwrap_or(0);
            if now - last < every {
                continue;
            }
            cache.reminded.insert(id.clone(), now);
            let shown: Vec<_> = missing.iter().take(3).cloned().collect();
            let extra = missing.len().saturating_sub(3);
            let more = if extra > 0 { format!(" y {extra} más") } else { String::new() };
            lines.push(format!("{label}: {}{more}", shown.join(", ")));
            ids.push(id);
        }
    }
    if lines.is_empty() {
        return;
    }

    let title = if lines.len() == 1 {
        "Un ticket tiene campos sin llenar".to_owned()
    } else {
        format!("{} tickets tienen campos sin llenar", lines.len())
    };
    let body = lines.iter().take(4).cloned().collect::<Vec<_>>().join("\n");
    if let Err(e) = app.notification().builder().title(&title).body(&body).show() {
        log::warn!("notificación: {e}");
    }
    let _ = app.emit(EVENT_REMINDER, &ids);
    if s.reminder_popup {
        crate::show_main(app, Some("pending"));
    }
}
