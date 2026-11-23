//! Token de ClickUp y contraseña SMTP en el llavero del sistema operativo
//! (Keychain en macOS, Credential Manager en Windows, Secret Service en Linux).
//! Nunca se escriben en disco en texto plano.

use keyring::Entry;

use crate::error::AppResult;

const SERVICE: &str = "co.tickets-ne.ticketpilot";
pub const CLICKUP_TOKEN: &str = "clickup_token";
pub const SMTP_PASSWORD: &str = "smtp_password";

pub fn get(key: &str) -> AppResult<Option<String>> {
    match Entry::new(SERVICE, key)?.get_password() {
        Ok(v) => Ok(Some(v)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

pub fn set(key: &str, value: &str) -> AppResult<()> {
    Entry::new(SERVICE, key)?.set_password(value)?;
    Ok(())
}

pub fn delete(key: &str) -> AppResult<()> {
    match Entry::new(SERVICE, key)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(e.into()),
    }
}
