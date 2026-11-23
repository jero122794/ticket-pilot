//! Cliente mínimo de la API oficial v2 de ClickUp (no hay scraping: la API es
//! estable, respeta permisos y no se rompe cuando cambia la interfaz web).

pub mod models;

use std::time::Duration;

use reqwest::{Method, RequestBuilder, Response, StatusCode};
use serde::de::DeserializeOwned;
use serde_json::{json, Value};

use crate::error::{AppError, AppResult};
pub use models::*;

const BASE: &str = "https://api.clickup.com/api/v2";

#[derive(Clone)]
pub struct ClickUp {
    http: reqwest::Client,
    token: String,
}

pub struct TaskQuery<'a> {
    pub team_id: &'a str,
    pub user_id: u64,
    pub list_ids: &'a [String],
    pub include_closed: bool,
    pub date_done_gt: Option<i64>,
}

impl ClickUp {
    pub fn new(token: impl Into<String>) -> AppResult<Self> {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(25))
            .connect_timeout(Duration::from_secs(10))
            .user_agent("TicketPilot/0.1")
            .build()?;
        Ok(Self { http, token: token.into() })
    }

    fn req(&self, method: Method, path: &str) -> RequestBuilder {
        self.http
            .request(method, format!("{BASE}{path}"))
            .header("Authorization", &self.token)
    }

    async fn send<T: DeserializeOwned>(&self, rb: RequestBuilder) -> AppResult<T> {
        let res = rb.send().await?;
        let res = check(res).await?;
        Ok(res.json::<T>().await?)
    }

    async fn send_unit(&self, rb: RequestBuilder) -> AppResult<()> {
        let res = rb.send().await?;
        check(res).await.map(|_| ())
    }

    pub async fn me(&self) -> AppResult<User> {
        let env: UserEnvelope = self.send(self.req(Method::GET, "/user")).await?;
        Ok(env.user)
    }

    pub async fn teams(&self) -> AppResult<Vec<Team>> {
        let env: TeamsEnvelope = self.send(self.req(Method::GET, "/team")).await?;
        Ok(env.teams)
    }

    /// Tareas asignadas al usuario en todo el workspace, paginando de 100 en 100.
    pub async fn assigned_tasks(&self, q: &TaskQuery<'_>) -> AppResult<Vec<Task>> {
        let mut out = Vec::new();
        for page in 0..50u32 {
            let mut params: Vec<(String, String)> = vec![
                ("page".into(), page.to_string()),
                ("assignees[]".into(), q.user_id.to_string()),
                ("subtasks".into(), "true".into()),
                ("include_closed".into(), q.include_closed.to_string()),
            ];
            for l in q.list_ids {
                params.push(("list_ids[]".into(), l.clone()));
            }
            if let Some(ts) = q.date_done_gt {
                params.push(("date_done_gt".into(), ts.to_string()));
            }
            let rb = self
                .req(Method::GET, &format!("/team/{}/task", q.team_id))
                .query(&params);
            let page_data: TasksPage = self.send(rb).await?;
            let n = page_data.tasks.len();
            out.extend(page_data.tasks);
            if page_data.last_page.unwrap_or(n < 100) || n == 0 {
                break;
            }
        }
        Ok(out)
    }

    pub async fn task(&self, task_id: &str) -> AppResult<Task> {
        self.send(self.req(Method::GET, &format!("/task/{task_id}"))).await
    }

    /// Tareas, listas y carpetas compartidas con el usuario. La búsqueda de
    /// "asignadas a mí" del workspace no incluye lo que vive en carpetas sin acceso.
    pub async fn shared(&self, team_id: &str) -> AppResult<SharedHierarchy> {
        // Se lee como JSON libre: en la práctica los ids pueden llegar como número
        // o como objeto, aunque la documentación diga texto.
        let v: Value = self.send(self.req(Method::GET, &format!("/team/{team_id}/shared"))).await?;
        let shared = v.get("shared").unwrap_or(&v);
        log::debug!("forma de /shared: {}", shape(shared, 2));
        let ids = |key: &str| -> Vec<String> {
            shared
                .get(key)
                .and_then(Value::as_array)
                .map(|a| a.iter().filter_map(id_of).collect())
                .unwrap_or_default()
        };
        Ok(SharedHierarchy {
            tasks: ids("tasks"),
            lists: ids("lists").into_iter().map(|id| NamedRef { id, name: None }).collect(),
            folders: ids("folders").into_iter().map(|id| NamedRef { id, name: None }).collect(),
        })
    }

    pub async fn folder_lists(&self, folder_id: &str) -> AppResult<Vec<NamedRef>> {
        let env: ListsEnvelope = self.send(self.req(Method::GET, &format!("/folder/{folder_id}/list"))).await?;
        Ok(env.lists)
    }

    /// Tareas de una lista asignadas al usuario (paginando de 100 en 100).
    pub async fn list_tasks(&self, list_id: &str, user_id: u64, include_closed: bool, date_done_gt: Option<i64>) -> AppResult<Vec<Task>> {
        let mut out = Vec::new();
        for page in 0..20u32 {
            let mut params: Vec<(String, String)> = vec![
                ("page".into(), page.to_string()),
                ("assignees[]".into(), user_id.to_string()),
                ("subtasks".into(), "true".into()),
                ("include_closed".into(), include_closed.to_string()),
            ];
            if let Some(ts) = date_done_gt {
                params.push(("date_done_gt".into(), ts.to_string()));
            }
            let rb = self.req(Method::GET, &format!("/list/{list_id}/task")).query(&params);
            let p: TasksPage = self.send(rb).await?;
            let n = p.tasks.len();
            out.extend(p.tasks);
            if p.last_page.unwrap_or(n < 100) || n == 0 {
                break;
            }
        }
        Ok(out)
    }

    /// Tarea por su código personalizado (p. ej. "TKM-20742").
    pub async fn task_by_custom_id(&self, custom_id: &str, team_id: &str) -> AppResult<Task> {
        self.send(
            self.req(Method::GET, &format!("/task/{custom_id}"))
                .query(&[("custom_task_ids", "true"), ("team_id", team_id)]),
        )
        .await
    }

    pub async fn list_fields(&self, list_id: &str) -> AppResult<Vec<FieldDef>> {
        let env: FieldsEnvelope = self
            .send(self.req(Method::GET, &format!("/list/{list_id}/field")))
            .await?;
        Ok(env.fields)
    }

    /// Escribe un campo personalizado. Para fechas, `with_time` guarda también la hora.
    pub async fn set_field(&self, task_id: &str, field_id: &str, value: &Value, with_time: bool) -> AppResult<()> {
        let body = if with_time {
            json!({ "value": value, "value_options": { "time": true } })
        } else {
            json!({ "value": value })
        };
        self.send_unit(
            self.req(Method::POST, &format!("/task/{task_id}/field/{field_id}"))
                .json(&body),
        )
        .await
    }

    pub async fn set_status(&self, task_id: &str, status: &str) -> AppResult<()> {
        self.send_unit(
            self.req(Method::PUT, &format!("/task/{task_id}"))
                .json(&json!({ "status": status })),
        )
        .await
    }

    pub async fn comment(&self, task_id: &str, text: &str) -> AppResult<()> {
        self.send_unit(
            self.req(Method::POST, &format!("/task/{task_id}/comment"))
                .json(&json!({ "comment_text": text, "notify_all": false })),
        )
        .await
    }

    /// Registra un tramo de trabajo en el time tracking nativo de ClickUp,
    /// así el campo "Tiempo registrado" del ticket refleja las horas reales.
    pub async fn time_entry(&self, team_id: &str, task_id: &str, start_ms: i64, duration_ms: i64) -> AppResult<()> {
        self.send_unit(
            self.req(Method::POST, &format!("/team/{team_id}/time_entries"))
                .json(&json!({
                    "tid": task_id,
                    "start": start_ms,
                    "duration": duration_ms,
                    "description": "Registrado por TicketPilot",
                    "billable": false
                })),
        )
        .await
    }
}

/// Id de un elemento que puede venir como texto, número u objeto con "id".
fn id_of(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Object(o) => o.get("id").and_then(id_of),
        _ => None,
    }
}

/// Describe la estructura de un JSON (claves y tipos, sin valores) para diagnóstico.
fn shape(v: &Value, depth: usize) -> String {
    match v {
        Value::Object(o) if depth > 0 => format!(
            "{{{}}}",
            o.iter().map(|(k, x)| format!("{k}: {}", shape(x, depth - 1))).collect::<Vec<_>>().join(", ")
        ),
        Value::Object(_) => "objeto".into(),
        Value::Array(a) => format!("[{} × {}]", a.len(), a.first().map(|x| shape(x, depth.saturating_sub(1))).unwrap_or_else(|| "vacío".into())),
        Value::String(_) => "texto".into(),
        Value::Number(_) => "número".into(),
        Value::Bool(_) => "bool".into(),
        Value::Null => "null".into(),
    }
}

async fn check(res: Response) -> AppResult<Response> {
    let status = res.status();
    if status.is_success() {
        return Ok(res);
    }
    if status == StatusCode::TOO_MANY_REQUESTS {
        // X-RateLimit-Reset es un epoch en segundos.
        let now = chrono::Utc::now().timestamp();
        let wait = res
            .headers()
            .get("X-RateLimit-Reset")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<i64>().ok())
            .map(|reset| (reset - now).clamp(1, 120) as u64)
            .unwrap_or(30);
        return Err(AppError::RateLimited(wait));
    }
    let code = status.as_u16();
    let text = res.text().await.unwrap_or_default();
    // ClickUp responde 401 tanto por token inválido como por falta de acceso a
    // una carpeta o lista; solo lo primero obliga a reconfigurar el token.
    if status == StatusCode::UNAUTHORIZED && text.to_lowercase().contains("token") {
        return Err(AppError::Unauthorized);
    }
    let message = serde_json::from_str::<ErrorBody>(&text)
        .ok()
        .and_then(|b| match (b.err, b.ecode) {
            (Some(e), Some(c)) => Some(format!("{e} ({c})")),
            (Some(e), None) => Some(e),
            _ => None,
        })
        .unwrap_or(text);
    Err(AppError::ClickUp { status: code, message })
}
