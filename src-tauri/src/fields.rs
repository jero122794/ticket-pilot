//! Completar campos obligatorios desde la app y elegir cuáles le aplican al usuario.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::{AppHandle, State};

use crate::clickup::{norm, FieldDef, Task};
use crate::error::{AppError, AppResult};
use crate::state::{broadcast, missing_fields, AppState};
use crate::tracker::{self, Op};

/// Tipos que el formulario sabe editar; el resto se completa en ClickUp.
const SUPPORTED: &[&str] = &[
    "text", "short_text", "url", "email", "phone", "number", "currency", "date", "drop_down", "labels", "checkbox",
    "users",
];

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FieldOption {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditableField {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub options: Vec<FieldOption>,
    pub supported: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FillForm {
    pub task_id: String,
    pub label: String,
    pub name: String,
    pub url: String,
    pub me: Option<u64>,
    pub fields: Vec<EditableField>,
    /// Miembros del workspace, solo si hay campos de tipo usuario.
    pub members: Vec<FieldOption>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FieldInput {
    pub field_id: String,
    pub name: String,
    pub kind: String,
    /// Texto, número, ms (fecha), id de opción, lista de ids, booleano o id de usuario.
    pub value: Value,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RequiredField {
    pub name: String,
    pub applies: bool,
}

fn options_of(d: &FieldDef) -> Vec<FieldOption> {
    d.type_config
        .as_ref()
        .map(|c| {
            c.options
                .iter()
                .filter_map(|o| {
                    let name = o.name.as_deref().or(o.label.as_deref())?.trim().to_owned();
                    Some(FieldOption { id: o.id.clone(), name })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Definiciones de la lista o, si no hay acceso, las que trae el propio ticket.
async fn defs_for(state: &AppState, task: &Task) -> Vec<FieldDef> {
    match state.list_fields(&task.list.id).await {
        Ok(d) => d,
        Err(_) => task.custom_fields.iter().map(|f| f.to_def()).collect(),
    }
}

#[tauri::command]
pub async fn get_fill_form(state: State<'_, AppState>, task_id: String) -> AppResult<FillForm> {
    let s = state.settings();
    // Se lee el ticket en vivo: alguien pudo llenar campos desde la última recarga.
    let task = state.client().await?.task(&task_id).await?;
    let defs = defs_for(&state, &task).await;
    let missing = missing_fields(&task, Some(&defs), &s, &[s.fields.end_date.as_str()]);
    let fields: Vec<EditableField> = missing
        .iter()
        .filter_map(|name| {
            // Si el nombre se repite, se prefiere la copia editable.
            let mut same: Vec<&FieldDef> = defs.iter().filter(|d| norm(&d.name) == norm(name)).collect();
            same.sort_by_key(|d| !SUPPORTED.contains(&d.kind.as_str()));
            let d = same.first()?;
            Some(EditableField {
                id: d.id.clone(),
                name: d.name.trim().to_owned(),
                kind: d.kind.clone(),
                options: options_of(d),
                supported: SUPPORTED.contains(&d.kind.as_str()),
            })
        })
        .collect();
    let me = state.cache.lock().await.user.as_ref().map(|u| u.id);
    let members = if fields.iter().any(|f: &EditableField| f.kind == "users") {
        workspace_members(&state).await.unwrap_or_default()
    } else {
        vec![]
    };
    Ok(FillForm {
        task_id: task.id.clone(),
        label: task.display_id().to_owned(),
        name: task.name.clone(),
        url: task.url.clone(),
        me,
        fields,
        members,
    })
}

async fn workspace_members(state: &AppState) -> AppResult<Vec<FieldOption>> {
    let team_id = state.settings().team_id;
    let teams = state.client().await?.teams().await?;
    let team = teams
        .into_iter()
        .find(|t| Some(&t.id) == team_id.as_ref())
        .ok_or_else(|| AppError::invalid("No se encontró el workspace."))?;
    let mut out: Vec<FieldOption> = team
        .members
        .into_iter()
        .filter_map(|m| {
            let name = m.user.username.or(m.user.email)?;
            Some(FieldOption { id: m.user.id.to_string(), name })
        })
        .collect();
    out.sort_by_key(|o| o.name.to_lowercase());
    Ok(out)
}

/// Traduce el valor del formulario al formato de la API y al que se guarda en caché.
fn to_api(input: &FieldInput) -> AppResult<(Value, Value, bool)> {
    let v = &input.value;
    let bad = || AppError::invalid(format!("Valor no válido para «{}».", input.name));
    Ok(match input.kind.as_str() {
        "date" => {
            let ms = v.as_i64().ok_or_else(bad)?;
            (json!(ms), json!(ms.to_string()), true)
        }
        "number" | "currency" => {
            let n = v.as_f64().ok_or_else(bad)?;
            (json!(n), json!(n), false)
        }
        "checkbox" => {
            let b = v.as_bool().ok_or_else(bad)?;
            (json!(b), json!(b), false)
        }
        "labels" => {
            let ids = v.as_array().filter(|a| !a.is_empty()).ok_or_else(bad)?;
            (json!(ids), json!(ids), false)
        }
        "users" => {
            let id = v.as_u64().ok_or_else(bad)?;
            (json!({ "add": [id], "rem": [] }), json!([{ "id": id }]), false)
        }
        _ => {
            let t = v.as_str().map(str::trim).filter(|t| !t.is_empty()).ok_or_else(bad)?;
            (json!(t), json!(t), false)
        }
    })
}

#[tauri::command]
pub async fn save_fields(
    app: AppHandle,
    state: State<'_, AppState>,
    task_id: String,
    values: Vec<FieldInput>,
) -> AppResult<()> {
    let task = match state.cached_task(&task_id).await {
        Some(t) => t,
        None => state.client().await?.task(&task_id).await?,
    };
    let mut ops = Vec::new();
    let mut patches = Vec::new();
    for input in &values {
        let (api_value, cache_value, time) = to_api(input)?;
        ops.push(Op::SetRaw { field_id: input.field_id.clone(), name: input.name.clone(), value: api_value, time });
        patches.push((input.field_id.clone(), cache_value));
    }
    tracker::enqueue_all(&state, &task, ops)?;

    // La interfaz refleja el cambio de inmediato; la recarga posterior lo confirma.
    {
        let mut cache = state.cache.lock().await;
        if let Some(t) = cache.tasks.iter_mut().find(|t| t.id == task_id) {
            for (id, v) in patches {
                if let Some(f) = t.custom_fields.iter_mut().find(|f| f.id == id) {
                    f.value = Some(v);
                }
            }
        }
    }
    state.wake.notify_one();
    broadcast(&app, false).await;
    Ok(())
}

/// Todos los campos obligatorios de las listas de mis tickets, con si me aplican.
#[tauri::command]
pub async fn list_required_fields(state: State<'_, AppState>) -> AppResult<Vec<RequiredField>> {
    let s = state.settings();
    let ignored: Vec<String> = s.ignored_fields.iter().map(|x| norm(x)).collect();
    let cache = state.cache.lock().await;
    let mut names: Vec<String> = Vec::new();
    let mut add = |n: &str| {
        if !names.iter().any(|x| norm(x) == norm(n)) {
            names.push(n.trim().to_owned());
        }
    };
    for defs in cache.fields.values() {
        defs.iter().filter(|d| d.required.unwrap_or(false) && d.is_fillable()).for_each(|d| add(&d.name));
    }
    for t in &cache.tasks {
        t.custom_fields
            .iter()
            .filter(|f| f.required.unwrap_or(false) && !matches!(f.kind.as_str(), "formula" | "multi_key"))
            .for_each(|f| add(&f.name));
    }
    // Los ignorados que ya no existen se muestran igual, para poder reactivarlos.
    for i in &s.ignored_fields {
        add(i);
    }
    names.sort_by_key(|n| n.to_lowercase());
    Ok(names
        .into_iter()
        .map(|name| RequiredField { applies: !ignored.contains(&norm(&name)), name })
        .collect())
}

/// Marca un campo como "no me aplica" (o lo reactiva).
#[tauri::command]
pub async fn set_field_applies(app: AppHandle, state: State<'_, AppState>, name: String, applies: bool) -> AppResult<()> {
    let mut s = state.settings();
    s.ignored_fields.retain(|x| norm(x) != norm(&name));
    if !applies {
        s.ignored_fields.push(name.trim().to_owned());
    }
    state.save_settings(s)?;
    broadcast(&app, false).await;
    Ok(())
}

/// Extrae el id de un enlace o código de ClickUp:
/// "https://app.clickup.com/t/90133100517/TKM-20742" → ("TKM-20742", true)
/// "https://app.clickup.com/t/17tqamf9fuc" o "17tqamf9fuc" → ("17tqamf9fuc", false)
fn parse_ref(input: &str) -> Option<(String, bool)> {
    let s = input.trim().trim_end_matches('/');
    let last = s
        .split(['?', '#'])
        .next()?
        .rsplit('/')
        .next()?
        .trim();
    if last.is_empty() || last.contains(' ') {
        return None;
    }
    let is_custom = last
        .split_once('-')
        .is_some_and(|(p, n)| !p.is_empty() && p.chars().all(|c| c.is_ascii_alphabetic()) && n.chars().all(|c| c.is_ascii_digit()));
    Some((if is_custom { last.to_uppercase() } else { last.to_owned() }, is_custom))
}

/// Agrega un ticket por enlace o código y lo sigue en cada recarga.
#[tauri::command]
pub async fn add_task_by_ref(app: AppHandle, state: State<'_, AppState>, reference: String) -> AppResult<String> {
    let (id, custom) =
        parse_ref(&reference).ok_or_else(|| AppError::invalid("Pega el enlace del ticket o su código, por ejemplo TKM-20742."))?;
    let client = state.client().await?;
    let task = if custom {
        let team = state
            .settings()
            .team_id
            .ok_or_else(|| AppError::invalid("Aún no se conoce el workspace. Espera la primera sincronización."))?;
        client.task_by_custom_id(&id, &team).await?
    } else {
        client.task(&id).await?
    };
    let label = format!("{} {}", task.display_id(), task.name);

    let mut s = state.settings();
    if !s.extra_task_ids.contains(&task.id) {
        s.extra_task_ids.push(task.id.clone());
        state.save_settings(s)?;
    }
    {
        let mut cache = state.cache.lock().await;
        if !cache.tasks.iter().any(|t| t.id == task.id) {
            cache.tasks.push(task);
        }
    }
    state.request_refresh();
    broadcast(&app, false).await;
    Ok(label)
}

/// Deja de seguir un ticket agregado a mano.
#[tauri::command]
pub async fn remove_manual_task(app: AppHandle, state: State<'_, AppState>, task_id: String) -> AppResult<()> {
    let mut s = state.settings();
    s.extra_task_ids.retain(|x| x != &task_id);
    state.save_settings(s)?;
    state.cache.lock().await.tasks.retain(|t| t.id != task_id);
    broadcast(&app, false).await;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::parse_ref;

    #[test]
    fn enlaces_y_codigos() {
        assert_eq!(parse_ref("https://app.clickup.com/t/90133100517/TKM-20742"), Some(("TKM-20742".into(), true)));
        assert_eq!(parse_ref("https://app.clickup.com/t/17tqamf9fuc"), Some(("17tqamf9fuc".into(), false)));
        assert_eq!(parse_ref(" tkm-20742 "), Some(("TKM-20742".into(), true)));
        assert_eq!(parse_ref("https://app.clickup.com/t/86akepxj7?comment=1"), Some(("86akepxj7".into(), false)));
        assert_eq!(parse_ref(""), None);
    }
}
