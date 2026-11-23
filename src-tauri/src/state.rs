//! Estado compartido de la aplicación y la "foto" (snapshot) que ve el frontend.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::RwLock;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::{Mutex, Notify};

use crate::clickup::{norm, ClickUp, FieldDef, Task, User};
use crate::config::Settings;
use crate::db::{Db, OutboxFailure, Punch};
use crate::error::{AppError, AppResult};
use crate::secrets;

pub const EVENT_SNAPSHOT: &str = "snapshot";

#[derive(Default)]
pub struct Cache {
    pub client: Option<ClickUp>,
    pub user: Option<User>,
    /// Tickets abiertos asignados + cerrados recientes (deduplicados por id).
    pub tasks: Vec<Task>,
    /// Definiciones de campos por lista (para resolver nombre → id y opciones).
    pub fields: HashMap<String, Vec<FieldDef>>,
    pub last_sync: Option<i64>,
    pub last_error: Option<String>,
    /// Listas a las que el usuario no tiene acceso (tiene tickets ahí, pero no ve la carpeta).
    pub no_access: std::collections::HashSet<String>,
    /// Tareas compartidas que no hay que volver a leer hasta cierto momento (ms):
    /// no están asignadas al usuario o llevan mucho tiempo cerradas.
    pub shared_skip: HashMap<String, i64>,
    /// Último recordatorio enviado por ticket (ms).
    pub reminded: HashMap<String, i64>,
}

pub struct AppState {
    pub db: Db,
    pub config_dir: PathBuf,
    pub settings: RwLock<Settings>,
    pub cache: Mutex<Cache>,
    /// Despierta el bucle de fondo (tras Play/Pausa/Stop o un cambio de ajustes).
    pub wake: Notify,
    /// Pide una recarga de tickets en el próximo ciclo.
    pub dirty: std::sync::atomic::AtomicBool,
}

impl AppState {
    pub fn settings(&self) -> Settings {
        self.settings.read().unwrap_or_else(|p| p.into_inner()).clone()
    }

    pub fn save_settings(&self, s: Settings) -> AppResult<()> {
        s.save(&self.config_dir)?;
        *self.settings.write().unwrap_or_else(|p| p.into_inner()) = s;
        Ok(())
    }

    pub fn request_refresh(&self) {
        self.dirty.store(true, std::sync::atomic::Ordering::SeqCst);
        self.wake.notify_one();
    }

    /// Cliente de ClickUp con el token del llavero (se crea una sola vez).
    pub async fn client(&self) -> AppResult<ClickUp> {
        let mut cache = self.cache.lock().await;
        if let Some(c) = &cache.client {
            return Ok(c.clone());
        }
        let token = secrets::get(secrets::CLICKUP_TOKEN)?
            .ok_or_else(|| AppError::invalid("Falta el token personal de ClickUp. Configúralo en Ajustes."))?;
        let c = ClickUp::new(token)?;
        cache.client = Some(c.clone());
        Ok(c)
    }

    pub async fn reset_client(&self) {
        let mut cache = self.cache.lock().await;
        cache.client = None;
        cache.user = None;
    }

    pub async fn cached_task(&self, task_id: &str) -> Option<Task> {
        self.cache.lock().await.tasks.iter().find(|t| t.id == task_id).cloned()
    }

    /// Definiciones de campos de una lista, desde caché o la API.
    pub async fn list_fields(&self, list_id: &str) -> AppResult<Vec<FieldDef>> {
        if let Some(f) = self.cache.lock().await.fields.get(list_id) {
            return Ok(f.clone());
        }
        let fields = self.client().await?.list_fields(list_id).await?;
        self.cache.lock().await.fields.insert(list_id.to_owned(), fields.clone());
        Ok(fields)
    }
}

pub fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

// ---------------------------------------------------------------------------
// Clasificación de tickets
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Bucket {
    NotStarted,
    InProgress,
    Done,
    Reopened,
}

pub fn bucket(task: &Task, s: &Settings) -> Bucket {
    let st = norm(&task.status.status);
    let kind = task.status.kind.as_deref().unwrap_or("");
    if s.statuses.reopened.iter().any(|x| norm(x) == st) {
        Bucket::Reopened
    } else if kind == "closed" || kind == "done" || norm(&s.statuses.done) == st {
        Bucket::Done
    } else if s.statuses.not_started.iter().any(|x| norm(x) == st) || matches!(kind, "unstarted" | "open") {
        Bucket::NotStarted
    } else {
        Bucket::InProgress
    }
}

/// Campos obligatorios de la lista que el ticket no tiene llenos.
/// `skip` excluye los que TicketPilot va a llenar por sí mismo.
pub fn missing_fields(task: &Task, defs: Option<&Vec<FieldDef>>, s: &Settings, skip: &[&str]) -> Vec<String> {
    let ignored: Vec<String> = s.ignored_fields.iter().map(|x| norm(x)).collect();
    let skip: Vec<String> = skip.iter().map(|x| norm(x)).collect();
    let mut out: Vec<String> = Vec::new();
    let mut push = |name: &str| {
        let n = norm(name);
        if !ignored.contains(&n) && !skip.contains(&n) && !out.iter().any(|o| norm(o) == n) && !task.field_is_set(name) {
            out.push(name.trim().to_owned());
        }
    };
    // Las definiciones de la lista marcan cuáles son obligatorios; si aún no
    // están en caché, se usa el indicador que trae la propia tarea.
    match defs {
        Some(defs) => defs
            .iter()
            .filter(|d| d.required.unwrap_or(false) && d.is_fillable())
            .for_each(|d| push(&d.name)),
        None => task
            .custom_fields
            .iter()
            .filter(|f| f.required.unwrap_or(false) && !matches!(f.kind.as_str(), "formula" | "multi_key"))
            .for_each(|f| push(&f.name)),
    }
    // Un ticket reabierto debe documentar cuándo y por qué.
    if bucket(task, s) == Bucket::Reopened {
        push(&s.fields.reopened_date);
        push(&s.fields.reopened_reason);
    }
    out
}

// ---------------------------------------------------------------------------
// Snapshot para el frontend
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TicketView {
    pub id: String,
    pub label: String,
    pub name: String,
    pub status: String,
    pub status_color: Option<String>,
    pub url: String,
    pub list_name: String,
    pub priority: Option<String>,
    pub priority_color: Option<String>,
    pub due_date: Option<i64>,
    pub bucket: Bucket,
    pub missing_fields: Vec<String>,
    pub worked_ms: i64,
    pub session_state: Option<String>,
    /// Agregado a mano por enlace (no llega en la búsqueda de asignadas).
    pub manual: bool,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Counts {
    pub not_started: usize,
    pub in_progress: usize,
    pub done: usize,
    pub reopened: usize,
    pub with_missing: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CurrentView {
    pub task_id: String,
    pub label: String,
    pub name: String,
    pub url: String,
    pub state: String,
    /// Tiempo de tramos cerrados; el frontend suma `now - runningSince` en vivo.
    pub worked_ms: i64,
    pub running_since: Option<i64>,
    pub pause_count: i64,
    pub missing_fields: Vec<String>,
    /// Marcas de reloj recientes (inicio, pausas, reanudaciones).
    pub punches: Vec<Punch>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub configured: bool,
    pub user: Option<User>,
    pub last_sync: Option<i64>,
    pub last_error: Option<String>,
    pub syncing: bool,
    pub tickets: Vec<TicketView>,
    pub counts: Counts,
    pub current: Option<CurrentView>,
    pub pending_sync: i64,
    pub failures: Vec<OutboxFailure>,
    pub today_worked_ms: i64,
}

pub async fn snapshot(state: &AppState, syncing: bool) -> AppResult<Snapshot> {
    let s = state.settings();
    let now = now_ms();
    let worked: HashMap<String, i64> = state.db.worked_all(now)?.into_iter().collect();
    let paused: Vec<String> = state.db.paused_task_ids()?;
    let running = state.db.running_session()?;
    let configured = secrets::get(secrets::CLICKUP_TOKEN).ok().flatten().is_some();

    let cache = state.cache.lock().await;
    let mut counts = Counts::default();
    let mut tickets: Vec<TicketView> = cache
        .tasks
        .iter()
        .map(|t| {
            let b = bucket(t, &s);
            match b {
                Bucket::NotStarted => counts.not_started += 1,
                Bucket::InProgress => counts.in_progress += 1,
                Bucket::Done => counts.done += 1,
                Bucket::Reopened => counts.reopened += 1,
            }
            let missing = if b == Bucket::Done {
                vec![]
            } else {
                missing_fields(t, cache.fields.get(&t.list.id), &s, &[s.fields.end_date.as_str()])
            };
            if !missing.is_empty() {
                counts.with_missing += 1;
            }
            let session_state = if running.as_ref().is_some_and(|r| r.task_id == t.id) {
                Some("running".to_owned())
            } else if paused.contains(&t.id) {
                Some("paused".to_owned())
            } else {
                None
            };
            TicketView {
                id: t.id.clone(),
                label: t.display_id().to_owned(),
                name: t.name.clone(),
                status: t.status.status.clone(),
                status_color: t.status.color.clone(),
                url: t.url.clone(),
                list_name: t.list.name.clone().unwrap_or_default(),
                priority: t.priority.as_ref().and_then(|p| p.priority.clone()),
                priority_color: t.priority.as_ref().and_then(|p| p.color.clone()),
                due_date: t.due_date,
                bucket: b,
                missing_fields: missing,
                worked_ms: worked.get(&t.id).copied().unwrap_or(0),
                session_state,
                manual: s.extra_task_ids.contains(&t.id),
            }
        })
        .collect();
    // Orden: activo primero, luego pausados, reabiertos, en curso, por iniciar, finalizados.
    let rank = |t: &TicketView| match (t.session_state.as_deref(), t.bucket) {
        (Some("running"), _) => 0,
        (Some(_), _) => 1,
        (_, Bucket::Reopened) => 2,
        (_, Bucket::InProgress) => 3,
        (_, Bucket::NotStarted) => 4,
        (_, Bucket::Done) => 5,
    };
    tickets.sort_by(|a, b| rank(a).cmp(&rank(b)).then(a.due_date.unwrap_or(i64::MAX).cmp(&b.due_date.unwrap_or(i64::MAX))));

    let current = match state.db.current_session()? {
        Some(sess) => {
            let total = state.db.task_worked_ms(&sess.task_id, now)?;
            let open_start = state.db.open_segment_start(sess.id)?;
            let closed = total - open_start.map(|st| now - st).unwrap_or(0);
            let task = cache.tasks.iter().find(|t| t.id == sess.task_id);
            let missing = task
                .map(|t| missing_fields(t, cache.fields.get(&t.list.id), &s, &[s.fields.end_date.as_str()]))
                .unwrap_or_default();
            Some(CurrentView {
                pause_count: state.db.pause_count(&sess.task_id)?,
                punches: state.db.task_punches(&sess.task_id, 8)?,
                task_id: sess.task_id,
                label: sess.task_label,
                name: sess.task_name,
                url: sess.task_url,
                state: sess.state,
                worked_ms: closed,
                running_since: open_start,
                missing_fields: missing,
            })
        }
        None => None,
    };

    let day_start = local_day_start(now);
    let today_worked_ms = state.db.worked_by_task_between(day_start, now)?.iter().map(|(_, ms)| ms).sum();

    Ok(Snapshot {
        configured,
        user: cache.user.clone(),
        last_sync: cache.last_sync,
        last_error: cache.last_error.clone(),
        syncing,
        tickets,
        counts,
        current,
        pending_sync: state.db.outbox_pending()?,
        failures: state.db.outbox_failures()?,
        today_worked_ms,
    })
}

/// Medianoche local de hoy, en ms UTC.
pub fn local_day_start(now: i64) -> i64 {
    use chrono::{Local, TimeZone};
    let local = Local.timestamp_millis_opt(now).single().unwrap_or_else(Local::now);
    local
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .and_then(|d| Local.from_local_datetime(&d).earliest())
        .map(|d| d.timestamp_millis())
        .unwrap_or(now)
}

/// Envía la foto actual a todas las ventanas.
pub async fn broadcast(app: &AppHandle, syncing: bool) {
    let state = app.state::<AppState>();
    match snapshot(&state, syncing).await {
        Ok(snap) => {
            let _ = app.emit(EVENT_SNAPSHOT, snap);
        }
        Err(e) => log::warn!("snapshot: {e}"),
    }
}
