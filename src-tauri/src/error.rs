use serde::{Serialize, Serializer};

/// Error único de la aplicación. Se serializa como texto para que el frontend
/// lo muestre tal cual (los comandos de Tauri devuelven `Result<T, AppError>`).
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("ClickUp respondió {status}: {message}")]
    ClickUp { status: u16, message: String },
    #[error("El token de ClickUp no es válido o expiró. Configúralo de nuevo en Ajustes.")]
    Unauthorized,
    #[error("ClickUp limitó las peticiones; se reintentará en {0} s.")]
    RateLimited(u64),
    #[error("No hay conexión con ClickUp: {0}")]
    Http(#[from] reqwest::Error),
    #[error("Base de datos local: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("Llavero del sistema: {0}")]
    Keyring(#[from] keyring::Error),
    #[error("Correo: {0}")]
    Mail(String),
    #[error("Archivo: {0}")]
    Io(#[from] std::io::Error),
    #[error("Formato JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("{0}")]
    Invalid(String),
    #[error("Faltan campos obligatorios: {}", .0.join(", "))]
    MissingFields(Vec<String>),
}

impl AppError {
    pub fn invalid(msg: impl Into<String>) -> Self {
        AppError::Invalid(msg.into())
    }

    /// Errores que no se resuelven reintentando: el outbox los marca como fallidos.
    pub fn is_permanent(&self) -> bool {
        matches!(self, AppError::ClickUp { status, .. } if (400..500).contains(status) && *status != 429)
            || matches!(self, AppError::Invalid(_) | AppError::Json(_) | AppError::MissingFields(_))
    }
}

/// Al frontend llega `{ code, message, fields }` para poder reaccionar según el caso
/// (p. ej. abrir el diálogo de campos pendientes en lugar de un simple aviso).
impl Serialize for AppError {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let code = match self {
            AppError::Unauthorized => "unauthorized",
            AppError::RateLimited(_) => "rate_limited",
            AppError::Http(_) => "offline",
            AppError::MissingFields(_) => "missing_fields",
            AppError::Invalid(_) => "invalid",
            _ => "error",
        };
        let fields: &[String] = match self {
            AppError::MissingFields(f) => f,
            _ => &[],
        };
        let mut st = s.serialize_struct("AppError", 3)?;
        st.serialize_field("code", code)?;
        st.serialize_field("message", &self.to_string())?;
        st.serialize_field("fields", fields)?;
        st.end()
    }
}

pub type AppResult<T> = Result<T, AppError>;
