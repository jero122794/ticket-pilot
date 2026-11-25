//! Todo lo que antes obligaba a abrir ClickUp: detalle del ticket, comentarios,
//! cambio de estado, checklists, subtareas, registros de tiempo y sugerencias
//! de rama / merge request desde los repositorios locales.
//!
//! Nada de esto se guarda en la base local: se consulta a ClickUp al abrirlo.

use reqwest::Method;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::{AppHandle, State};

use crate::clickup::norm;
use crate::error::{AppError, AppResult};
use crate::state::{auto_filled, broadcast, missing_fields, now_ms, AppState};
use crate::tracker::{self, Op};

// ---------------------------------------------------------------- lectura tolerante

/// Texto de una clave (acepta números).
fn s(v: &Value, key: &str) -> String {
    match v.get(key) {
        Some(Value::String(x)) => x.clone(),
        Some(Value::Number(n)) => n.to_string(),
        _ => String::new(),
    }
}

/// Número de una clave (acepta texto).
fn n(v: &Value, key: &str) -> Option<i64> {
    match v.get(key)? {
        Value::Number(x) => x.as_i64().or_else(|| x.as_f64().map(|f| f as i64)),
        Value::String(x) => x.trim().parse().ok(),
        _ => None,
    }
}

fn arr<'a>(v: &'a Value, key: &str) -> &'a [Value] {
    v.get(key).and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[])
}

fn team_id(state: &AppState) -> AppResult<String> {
    state.settings().team_id.ok_or_else(|| AppError::invalid("Primero conecta ClickUp en Ajustes."))
}

// ---------------------------------------------------------------- detalle

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusOption {
    pub status: String,
    pub color: Option<String>,
    pub kind: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NamedValue {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DateValue {
    pub name: String,
    pub ms: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Attachment {
    pub id: String,
    pub title: String,
    pub url: String,
    pub thumbnail: Option<String>,
    pub extension: String,
    pub size: Option<i64>,
    pub at: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChecklistItem {
    pub id: String,
    pub name: String,
    pub resolved: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Checklist {
    pub id: String,
    pub name: String,
    pub items: Vec<ChecklistItem>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubtaskRef {
    pub id: String,
    pub label: String,
    pub name: String,
    pub status: String,
    pub status_color: Option<String>,
    pub done: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskDetail {
    pub id: String,
    pub label: String,
    pub name: String,
    pub url: String,
    pub status: String,
    pub status_color: Option<String>,
    /// Estados de la lista (vacío si no hay acceso a la lista).
    pub statuses: Vec<StatusOption>,
    pub description: String,
    /// Campos de texto con valor (requerimiento, criterios, causa raíz…).
    pub texts: Vec<NamedValue>,
    /// Campos de opción y número con valor (prioridad, proyecto, ambiente…).
    pub values: Vec<NamedValue>,
    pub dates: Vec<DateValue>,
    pub sla_due: Option<i64>,
    pub delivery_due: Option<i64>,
    pub assignees: Vec<String>,
    pub attachments: Vec<Attachment>,
    pub checklists: Vec<Checklist>,
    pub subtasks: Vec<SubtaskRef>,
    pub parent_id: Option<String>,
    /// Tiempo registrado en ClickUp por todas las personas.
    pub time_spent_ms: i64,
    pub estimate_ms: Option<i64>,
    pub missing_fields: Vec<String>,
    /// running | paused | null
    pub session_state: Option<String>,
    /// Campo de evidencias de pruebas («Observaciones»), aunque esté vacío.
    pub evidence: Option<EvidenceField>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceField {
    pub id: String,
    pub name: String,
    pub value: String,
}

/// Valor legible de un campo personalizado (opción, etiquetas, personas, número…).
fn field_display(f: &Value) -> Option<String> {
    let v = f.get("value")?;
    if v.is_null() {
        return None;
    }
    let kind = s(f, "type");
    let options = f.get("type_config").map(|c| arr(c, "options")).unwrap_or(&[]);
    let opt_name = |id_or_idx: &Value| -> Option<String> {
        let found = match id_or_idx {
            Value::Number(i) => options.iter().find(|o| n(o, "orderindex") == i.as_i64()),
            Value::String(id) => options.iter().find(|o| &s(o, "id") == id),
            Value::Object(_) => return Some(s(id_or_idx, "name")).filter(|x| !x.is_empty()).or_else(|| Some(s(id_or_idx, "label"))),
            _ => None,
        }?;
        Some(if s(found, "name").is_empty() { s(found, "label") } else { s(found, "name") })
    };
    let out = match kind.as_str() {
        "drop_down" => opt_name(v)?,
        "labels" => v.as_array()?.iter().filter_map(opt_name).collect::<Vec<_>>().join(", "),
        "users" => v
            .as_array()?
            .iter()
            .map(|u| s(u, "username"))
            .filter(|x| !x.is_empty())
            .collect::<Vec<_>>()
            .join(", "),
        "checkbox" => match v {
            Value::Bool(true) => "Sí".into(),
            Value::String(x) if x == "true" => "Sí".into(),
            _ => "No".into(),
        },
        "number" | "currency" | "formula" => match v {
            Value::Number(x) => x.to_string(),
            Value::String(x) => x.clone(),
            _ => return None,
        },
        "short_text" | "url" | "email" | "phone" => v.as_str()?.to_owned(),
        _ => return None,
    };
    Some(out).filter(|x| !x.trim().is_empty())
}

#[tauri::command]
pub async fn get_task_detail(state: State<'_, AppState>, task_id: String) -> AppResult<TaskDetail> {
    let st = state.settings();
    let client = state.client().await?;
    let t = client
        .raw(
            Method::GET,
            &format!("/task/{task_id}"),
            &[("include_subtasks", "true".into()), ("include_markdown_description", "true".into())],
            None,
        )
        .await?;
    let id = s(&t, "id");
    let list_id = t.get("list").map(|l| s(l, "id")).unwrap_or_default();

    let statuses = match client.raw(Method::GET, &format!("/list/{list_id}"), &[], None).await {
        Ok(l) => arr(&l, "statuses")
            .iter()
            .map(|x| StatusOption { status: s(x, "status"), color: Some(s(x, "color")).filter(|c| !c.is_empty()), kind: s(x, "type") })
            .collect(),
        Err(_) => vec![],
    };

    let mut description = s(&t, "markdown_description");
    if description.trim().is_empty() {
        description = s(&t, "text_content");
    }

    let mut texts = Vec::new();
    let mut values = Vec::new();
    let mut dates = Vec::new();
    let mut seen_text = Vec::<String>::new();
    let mut evidence = None;
    for f in arr(&t, "custom_fields") {
        let name = s(f, "name").trim().to_owned();
        if evidence.is_none() && norm(&name) == norm(&st.fields.evidence) && s(f, "type") == "text" {
            let value = f.get("value").and_then(Value::as_str).unwrap_or_default().to_owned();
            evidence = Some(EvidenceField { id: s(f, "id"), name: name.clone(), value });
            continue;
        }
        match s(f, "type").as_str() {
            "text" => {
                let v = f.get("value").and_then(Value::as_str).unwrap_or_default().trim().to_owned();
                // Hay campos duplicados con el mismo texto («Descripción del Requerimiento»).
                if !v.is_empty() && !seen_text.contains(&v) {
                    seen_text.push(v.clone());
                    texts.push(NamedValue { name, value: v });
                }
            }
            "date" => {
                if let Some(ms) = n(f, "value") {
                    dates.push(DateValue { name, ms });
                }
            }
            "formula" | "multi_key" => {}
            _ => {
                if let Some(v) = field_display(f) {
                    if !values.iter().any(|x: &NamedValue| norm(&x.name) == norm(&name)) {
                        values.push(NamedValue { name, value: v });
                    }
                }
            }
        }
    }
    dates.sort_by_key(|d| d.ms);
    let due = |field: &str| dates.iter().find(|d| norm(&d.name) == norm(field)).map(|d| d.ms);

    let attachments = arr(&t, "attachments")
        .iter()
        .map(|a| Attachment {
            id: s(a, "id"),
            title: s(a, "title"),
            url: s(a, "url"),
            thumbnail: [s(a, "thumbnail_large"), s(a, "thumbnail_medium"), s(a, "thumbnail_small")]
                .into_iter()
                .find(|x| !x.is_empty()),
            extension: s(a, "extension"),
            size: n(a, "size"),
            at: n(a, "date"),
        })
        .collect();

    let checklists = arr(&t, "checklists")
        .iter()
        .map(|c| Checklist {
            id: s(c, "id"),
            name: s(c, "name"),
            items: arr(c, "items")
                .iter()
                .map(|i| ChecklistItem { id: s(i, "id"), name: s(i, "name"), resolved: i.get("resolved").and_then(Value::as_bool).unwrap_or(false) })
                .collect(),
        })
        .collect();

    let subtasks = arr(&t, "subtasks")
        .iter()
        .map(|x| {
            let status = x.get("status").cloned().unwrap_or(Value::Null);
            let kind = s(&status, "type");
            let custom = s(x, "custom_id");
            SubtaskRef {
                id: s(x, "id"),
                label: if custom.is_empty() { s(x, "id") } else { custom },
                name: s(x, "name"),
                status: s(&status, "status"),
                status_color: Some(s(&status, "color")).filter(|c| !c.is_empty()),
                done: matches!(kind.as_str(), "done" | "closed"),
            }
        })
        .collect();

    let assignees = arr(&t, "assignees").iter().map(|u| s(u, "username")).filter(|x| !x.is_empty()).collect();
    let status = t.get("status").cloned().unwrap_or(Value::Null);
    let custom = s(&t, "custom_id");

    // Faltantes y estado del cronómetro: con el ticket tipado (caché o en vivo).
    let typed = serde_json::from_value::<crate::clickup::Task>(t.clone()).ok();
    let session = state.db.open_session_for_task(&id)?;
    let missing = match &typed {
        Some(task) => {
            let defs = state.list_fields(&task.list.id).await.ok();
            let skip: Vec<&str> = if session.is_some() { auto_filled(&st).to_vec() } else { vec![st.fields.end_date.as_str()] };
            missing_fields(task, defs.as_ref(), &st, &skip)
        }
        None => vec![],
    };

    Ok(TaskDetail {
        label: if custom.is_empty() { id.clone() } else { custom },
        name: s(&t, "name"),
        url: s(&t, "url"),
        status: s(&status, "status"),
        status_color: Some(s(&status, "color")).filter(|c| !c.is_empty()),
        statuses,
        description,
        texts,
        values,
        sla_due: due(&st.fields.sla_due),
        delivery_due: due(&st.fields.delivery_due),
        dates,
        assignees,
        attachments,
        checklists,
        subtasks,
        parent_id: Some(s(&t, "parent")).filter(|p| !p.is_empty()),
        time_spent_ms: n(&t, "time_spent").unwrap_or(0),
        estimate_ms: typed.as_ref().and_then(|x| x.estimate_ms(&st.fields.estimate)),
        missing_fields: missing,
        session_state: session.map(|x| x.state),
        evidence,
        id,
    })
}

// ---------------------------------------------------------------- evidencias

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Uploaded {
    pub url: String,
    pub title: String,
}

/// Máximo por archivo (ClickUp acepta más, pero un pantallazo no debería pasar de esto).
const MAX_UPLOAD: usize = 25 * 1024 * 1024;

/// Sube un pantallazo (base64) como adjunto del ticket y devuelve su enlace para
/// escribirlo en el campo de evidencias. No pasa por el disco ni por la base local.
#[tauri::command]
pub async fn upload_evidence(state: State<'_, AppState>, task_id: String, filename: String, mime: String, data: String) -> AppResult<Uploaded> {
    use base64::Engine;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data.trim())
        .map_err(|_| AppError::invalid("El archivo no se pudo leer."))?;
    if bytes.is_empty() {
        return Err(AppError::invalid("El archivo está vacío."));
    }
    if bytes.len() > MAX_UPLOAD {
        return Err(AppError::invalid("El archivo pasa de 25 MB."));
    }
    let clean: String = filename
        .chars()
        .map(|c| if c.is_alphanumeric() || matches!(c, '.' | '-' | '_') { c } else { '_' })
        .collect();
    let name = if clean.trim_matches('_').is_empty() { "evidencia.png".to_owned() } else { clean };
    let mime = if mime.is_empty() { "application/octet-stream".to_owned() } else { mime };
    let v = state.client().await?.upload_attachment(&task_id, &name, &mime, bytes).await?;
    let url = s(&v, "url");
    if url.is_empty() {
        return Err(AppError::invalid("ClickUp no devolvió el enlace del adjunto."));
    }
    Ok(Uploaded { url, title: Some(s(&v, "title")).filter(|x| !x.is_empty()).unwrap_or(name) })
}

// ---------------------------------------------------------------- comentarios

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Comment {
    pub id: String,
    pub text: String,
    pub user: String,
    pub user_id: Option<i64>,
    pub mine: bool,
    pub at: i64,
    pub replies: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Member {
    pub id: u64,
    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommentPart {
    #[serde(default)]
    pub text: Option<String>,
    /// Mención a una persona.
    #[serde(default)]
    pub user_id: Option<u64>,
}

pub fn parse_comments(v: &Value, me: Option<u64>) -> Vec<Comment> {
    let mut out: Vec<Comment> = arr(v, "comments")
        .iter()
        .map(|c| {
            let user = c.get("user").cloned().unwrap_or(Value::Null);
            let uid = n(&user, "id");
            Comment {
                id: s(c, "id"),
                text: s(c, "comment_text"),
                user: s(&user, "username"),
                user_id: uid,
                mine: uid.is_some() && uid == me.map(|m| m as i64),
                at: n(c, "date").unwrap_or(0),
                replies: n(c, "reply_count").unwrap_or(0),
            }
        })
        .collect();
    out.sort_by_key(|c| c.at);
    out
}

#[tauri::command]
pub async fn get_comments(state: State<'_, AppState>, task_id: String) -> AppResult<Vec<Comment>> {
    let v = state.client().await?.raw(Method::GET, &format!("/task/{task_id}/comment"), &[], None).await?;
    let me = state.cache.lock().await.user.as_ref().map(|u| u.id);
    Ok(parse_comments(&v, me))
}

#[tauri::command]
pub async fn get_task_members(state: State<'_, AppState>, task_id: String) -> AppResult<Vec<Member>> {
    let v = state.client().await?.raw(Method::GET, &format!("/task/{task_id}/member"), &[], None).await?;
    let mut out: Vec<Member> = arr(&v, "members")
        .iter()
        .filter_map(|m| {
            let id = n(m, "id")? as u64;
            let name = Some(s(m, "username")).filter(|x| !x.is_empty()).unwrap_or_else(|| s(m, "email"));
            Some(Member { id, name })
        })
        .collect();
    out.sort_by_key(|m| m.name.to_lowercase());
    Ok(out)
}

/// Publica un comentario con menciones. Se envía en el momento (no va a la cola):
/// si no hay conexión, la interfaz conserva el borrador y muestra el error.
#[tauri::command]
pub async fn add_comment(state: State<'_, AppState>, task_id: String, parts: Vec<CommentPart>) -> AppResult<()> {
    let mut comment = Vec::new();
    let mut plain = String::new();
    for p in &parts {
        if let Some(uid) = p.user_id {
            comment.push(json!({ "type": "tag", "user": { "id": uid } }));
        } else if let Some(t) = p.text.as_deref().filter(|t| !t.is_empty()) {
            comment.push(json!({ "text": t }));
            plain.push_str(t);
        }
    }
    if plain.trim().is_empty() && !parts.iter().any(|p| p.user_id.is_some()) {
        return Err(AppError::invalid("Escribe el comentario."));
    }
    state
        .client()
        .await?
        .raw(Method::POST, &format!("/task/{task_id}/comment"), &[], Some(json!({ "comment": comment, "notify_all": false })))
        .await?;
    Ok(())
}

// ---------------------------------------------------------------- estado

/// Cambia el estado del ticket (va por la cola, como Play y Finalizar). Con motivo,
/// también deja un comentario.
#[tauri::command]
pub async fn set_task_status(
    app: AppHandle,
    state: State<'_, AppState>,
    task_id: String,
    status: String,
    note: String,
) -> AppResult<()> {
    let st = state.settings();
    if norm(&status) == norm(&st.statuses.done) && state.db.open_session_for_task(&task_id)?.is_some() {
        return Err(AppError::invalid("Para pasarlo a «listo para release» usa Finalizar: así se cierra el cronómetro y se exigen los campos."));
    }
    let task = match state.cached_task(&task_id).await {
        Some(t) => t,
        None => state.client().await?.task(&task_id).await?,
    };
    let mut ops = vec![Op::Status { status: status.clone() }];
    let note = note.trim();
    if !note.is_empty() {
        ops.push(Op::Comment { text: format!("Estado → {status}\nMotivo: {note}") });
    }
    tracker::enqueue_all(&state, &task, ops)?;
    {
        let mut cache = state.cache.lock().await;
        if let Some(t) = cache.tasks.iter_mut().find(|t| t.id == task_id) {
            t.status.status = status;
        }
    }
    state.wake.notify_one();
    broadcast(&app, false).await;
    Ok(())
}

// ---------------------------------------------------------------- checklist

#[tauri::command]
pub async fn set_checklist_item(state: State<'_, AppState>, checklist_id: String, item_id: String, resolved: bool) -> AppResult<()> {
    state
        .client()
        .await?
        .raw(
            Method::PUT,
            &format!("/checklist/{checklist_id}/checklist_item/{item_id}"),
            &[],
            Some(json!({ "resolved": resolved })),
        )
        .await?;
    Ok(())
}

// ---------------------------------------------------------------- tiempo

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TimeEntry {
    pub id: String,
    pub start: i64,
    pub end: i64,
    pub duration: i64,
    pub description: String,
    /// El cronómetro de ClickUp sigue corriendo.
    pub running: bool,
}

#[tauri::command]
pub async fn get_time_entries(state: State<'_, AppState>, task_id: String) -> AppResult<Vec<TimeEntry>> {
    let team = team_id(&state)?;
    let now = now_ms();
    let v = state
        .client()
        .await?
        .raw(
            Method::GET,
            &format!("/team/{team}/time_entries"),
            &[
                ("task_id", task_id.clone()),
                ("start_date", (now - 365 * 86_400_000).to_string()),
                ("end_date", now.to_string()),
            ],
            None,
        )
        .await?;
    let mut out: Vec<TimeEntry> = arr(&v, "data")
        .iter()
        .filter(|e| e.get("task").map(|t| s(t, "id")) == Some(task_id.clone()))
        .filter_map(|e| {
            let start = n(e, "start")?;
            let duration = n(e, "duration").unwrap_or(0);
            let running = duration < 0;
            let end = if running { now } else { n(e, "end").unwrap_or(start + duration) };
            Some(TimeEntry { id: s(e, "id"), start, end, duration: end - start, description: s(e, "description"), running })
        })
        .collect();
    out.sort_by_key(|e| std::cmp::Reverse(e.start));
    Ok(out)
}

fn check_span(start: i64, end: i64) -> AppResult<()> {
    if end <= start {
        return Err(AppError::invalid("La hora de fin debe ser posterior a la de inicio."));
    }
    if end > now_ms() + 60_000 {
        return Err(AppError::invalid("No se puede registrar tiempo en el futuro."));
    }
    if end - start > 16 * 3_600_000 {
        return Err(AppError::invalid("Un registro no puede pasar de 16 horas."));
    }
    Ok(())
}

#[tauri::command]
pub async fn add_time_entry(state: State<'_, AppState>, task_id: String, start: i64, end: i64, note: String) -> AppResult<()> {
    check_span(start, end)?;
    let team = team_id(&state)?;
    let description = if note.trim().is_empty() { "Agregado a mano en TicketPilot".to_owned() } else { note.trim().to_owned() };
    state
        .client()
        .await?
        .raw(
            Method::POST,
            &format!("/team/{team}/time_entries"),
            &[],
            Some(json!({ "tid": task_id, "start": start, "duration": end - start, "description": description, "billable": false })),
        )
        .await?;
    Ok(())
}

#[tauri::command]
pub async fn update_time_entry(state: State<'_, AppState>, task_id: String, entry_id: String, start: i64, end: i64) -> AppResult<()> {
    check_span(start, end)?;
    let team = team_id(&state)?;
    state
        .client()
        .await?
        .raw(
            Method::PUT,
            &format!("/team/{team}/time_entries/{entry_id}"),
            &[],
            Some(json!({ "tid": task_id, "start": start, "end": end, "duration": end - start })),
        )
        .await?;
    Ok(())
}

#[tauri::command]
pub async fn delete_time_entry(state: State<'_, AppState>, entry_id: String) -> AppResult<()> {
    let team = team_id(&state)?;
    state
        .client()
        .await?
        .raw(Method::DELETE, &format!("/team/{team}/time_entries/{entry_id}"), &[], None)
        .await?;
    Ok(())
}

// ---------------------------------------------------------------- git

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitHint {
    pub repo: String,
    pub branch: String,
    pub branch_url: Option<String>,
    pub mr_url: Option<String>,
    /// La rama contiene el código del ticket.
    pub matches: bool,
}

fn git(dir: &str, args: &[&str]) -> Option<String> {
    let out = std::process::Command::new("git").arg("-C").arg(dir).args(args).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_owned()).filter(|x| !x.is_empty())
}

/// `git@gitlab.com:grupo/repo.git` o `https://user@host/grupo/repo.git` → `https://host/grupo/repo`.
pub fn web_url(remote: &str) -> Option<String> {
    let r = remote.trim().trim_end_matches('/').trim_end_matches(".git");
    if let Some(rest) = r.strip_prefix("git@") {
        let (host, path) = rest.split_once(':')?;
        return Some(format!("https://{host}/{path}"));
    }
    if let Some(rest) = r.strip_prefix("ssh://") {
        let rest = rest.rsplit_once('@').map(|(_, x)| x).unwrap_or(rest);
        let (host, path) = rest.split_once('/')?;
        let host = host.split(':').next()?;
        return Some(format!("https://{host}/{path}"));
    }
    let rest = r.strip_prefix("https://").or_else(|| r.strip_prefix("http://"))?;
    let rest = match rest.split_once('/') {
        Some((auth_host, path)) => format!("{}/{path}", auth_host.rsplit_once('@').map(|(_, h)| h).unwrap_or(auth_host)),
        None => return None,
    };
    Some(format!("https://{rest}"))
}

#[tauri::command]
pub async fn git_suggestions(state: State<'_, AppState>, label: String) -> AppResult<Vec<GitHint>> {
    let paths = state.settings().repo_paths;
    let wanted = label.to_lowercase();
    let hints = tauri::async_runtime::spawn_blocking(move || {
        let mut out = Vec::new();
        for dir in paths.iter().map(|p| p.trim()).filter(|p| !p.is_empty()) {
            let Some(branch) = git(dir, &["rev-parse", "--abbrev-ref", "HEAD"]) else { continue };
            let web = git(dir, &["remote", "get-url", "origin"]).and_then(|r| web_url(&r));
            let github = web.as_deref().is_some_and(|w| w.contains("github.com"));
            let enc = branch.replace('#', "%23");
            let repo = std::path::Path::new(dir).file_name().map(|x| x.to_string_lossy().into_owned()).unwrap_or_else(|| dir.to_owned());
            out.push(GitHint {
                matches: !wanted.is_empty() && branch.to_lowercase().contains(&wanted),
                branch_url: web.as_ref().map(|w| if github { format!("{w}/tree/{enc}") } else { format!("{w}/-/tree/{enc}") }),
                mr_url: web.as_ref().map(|w| {
                    if github {
                        format!("{w}/pulls?q=is%3Apr+head%3A{enc}")
                    } else {
                        format!("{w}/-/merge_requests?scope=all&state=all&source_branch={enc}")
                    }
                }),
                repo,
                branch,
            });
        }
        out.sort_by_key(|h| !h.matches);
        out
    })
    .await
    .map_err(|e| AppError::invalid(e.to_string()))?;
    Ok(hints)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls_de_remotos() {
        assert_eq!(web_url("git@gitlab.com:grupo/app.git").as_deref(), Some("https://gitlab.com/grupo/app"));
        assert_eq!(web_url("https://user:tok@gitlab.empresa.co/a/b.git").as_deref(), Some("https://gitlab.empresa.co/a/b"));
        assert_eq!(web_url("ssh://git@host.co:2222/a/b.git").as_deref(), Some("https://host.co/a/b"));
        assert_eq!(web_url("https://github.com/x/y").as_deref(), Some("https://github.com/x/y"));
    }
}
