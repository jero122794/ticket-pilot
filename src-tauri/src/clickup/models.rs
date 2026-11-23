//! Modelos de la API v2 de ClickUp. Son tolerantes: la API devuelve números
//! como texto en algunos campos (fechas en ms) y omite claves vacías.

use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: u64,
    pub username: String,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub color: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct UserEnvelope {
    pub user: User,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Team {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing)]
    pub members: Vec<Member>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Member {
    pub user: MemberUser,
}

/// En /team los invitados pendientes llegan sin nombre de usuario.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemberUser {
    pub id: u64,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct TeamsEnvelope {
    pub teams: Vec<Team>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Status {
    pub status: String,
    #[serde(default)]
    pub color: Option<String>,
    /// open | custom | done | closed (y "unstarted" en listas nuevas).
    #[serde(default, rename = "type")]
    pub kind: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NamedRef {
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Priority {
    #[serde(default)]
    pub priority: Option<String>,
    #[serde(default)]
    pub color: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskField {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub value: Option<Value>,
    #[serde(default)]
    pub required: Option<bool>,
    #[serde(default)]
    pub type_config: Option<FieldTypeConfig>,
}

impl TaskField {
    /// Definición equivalente, para listas cuyas definiciones no se pueden leer.
    pub fn to_def(&self) -> FieldDef {
        FieldDef {
            id: self.id.clone(),
            name: self.name.clone(),
            kind: self.kind.clone(),
            required: self.required,
            type_config: self.type_config.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    #[serde(default)]
    pub custom_id: Option<String>,
    pub name: String,
    pub status: Status,
    pub url: String,
    pub list: NamedRef,
    #[serde(default)]
    pub priority: Option<Priority>,
    #[serde(default, deserialize_with = "de_opt_i64")]
    pub due_date: Option<i64>,
    #[serde(default, deserialize_with = "de_opt_i64")]
    pub date_updated: Option<i64>,
    #[serde(default, deserialize_with = "de_opt_i64")]
    pub date_done: Option<i64>,
    #[serde(default, deserialize_with = "de_opt_i64")]
    pub time_spent: Option<i64>,
    #[serde(default)]
    pub custom_fields: Vec<TaskField>,
    #[serde(default)]
    pub assignees: Vec<UserRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserRef {
    pub id: u64,
}

impl Task {
    /// Un nombre puede repetirse (p. ej. "Descripción del Requerimiento" existe dos
    /// veces en el workspace): basta con que una de las copias tenga valor.
    pub fn field_is_set(&self, name: &str) -> bool {
        let wanted = norm(name);
        self.custom_fields
            .iter()
            .any(|f| norm(&f.name) == wanted && has_value(&f.value))
    }

    pub fn field_text(&self, name: &str) -> Option<String> {
        let wanted = norm(name);
        self.custom_fields
            .iter()
            .filter(|f| norm(&f.name) == wanted)
            .find_map(|f| f.value.as_ref()?.as_str().map(str::to_owned))
    }

    pub fn display_id(&self) -> &str {
        self.custom_id.as_deref().unwrap_or(&self.id)
    }
}

/// Lo compartido con el usuario: tareas sueltas (ids), listas y carpetas.
#[derive(Debug, Clone, Default)]
pub struct SharedHierarchy {
    pub tasks: Vec<String>,
    pub lists: Vec<NamedRef>,
    pub folders: Vec<NamedRef>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ListsEnvelope {
    #[serde(default)]
    pub lists: Vec<NamedRef>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct TasksPage {
    pub tasks: Vec<Task>,
    #[serde(default)]
    pub last_page: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DropdownOption {
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FieldTypeConfig {
    #[serde(default)]
    pub options: Vec<DropdownOption>,
}

/// Definición de un campo personalizado a nivel de lista.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldDef {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub required: Option<bool>,
    #[serde(default)]
    pub type_config: Option<FieldTypeConfig>,
}

impl FieldDef {
    pub fn option_id(&self, option_name: &str) -> Option<String> {
        let wanted = norm(option_name);
        self.type_config.as_ref()?.options.iter().find_map(|o| {
            let n = o.name.as_deref().or(o.label.as_deref())?;
            (norm(n) == wanted).then(|| o.id.clone())
        })
    }

    /// Campos que una persona puede llenar (las fórmulas y baselines se calculan solos).
    pub fn is_fillable(&self) -> bool {
        !matches!(self.kind.as_str(), "formula" | "multi_key" | "rollup" | "button" | "progress" | "automatic_progress")
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct FieldsEnvelope {
    pub fields: Vec<FieldDef>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ErrorBody {
    #[serde(default)]
    pub err: Option<String>,
    #[serde(default, rename = "ECODE")]
    pub ecode: Option<String>,
}

/// Compara nombres de campos y estados sin importar mayúsculas ni espacios
/// sobrantes (en el workspace hay opciones como "Alta " con espacio final).
pub fn norm(s: &str) -> String {
    s.trim().to_lowercase()
}

pub fn has_value(v: &Option<Value>) -> bool {
    match v {
        None | Some(Value::Null) => false,
        Some(Value::String(s)) => !s.trim().is_empty(),
        Some(Value::Array(a)) => !a.is_empty(),
        Some(Value::Object(o)) => !o.is_empty(),
        Some(_) => true,
    }
}

fn de_opt_i64<'de, D: Deserializer<'de>>(d: D) -> Result<Option<i64>, D::Error> {
    Ok(match Option::<Value>::deserialize(d)? {
        Some(Value::Number(n)) => n.as_i64(),
        Some(Value::String(s)) => s.parse().ok(),
        _ => None,
    })
}
