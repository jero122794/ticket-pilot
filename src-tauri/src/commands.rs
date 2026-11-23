//! Comandos invocables desde el frontend. Las acciones devuelven la foto
//! actualizada para que la interfaz responda sin esperar al bucle de fondo.

use tauri::{AppHandle, Manager, State};
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_opener::OpenerExt;

use crate::clickup::{ClickUp, Team, User};
use crate::config::Settings;
use crate::error::{AppError, AppResult};
use crate::state::{broadcast, now_ms, snapshot, AppState, Snapshot};
use crate::{mail, secrets, tracker};

async fn after_action(app: &AppHandle, state: &AppState) -> AppResult<Snapshot> {
    state.request_refresh();
    broadcast(app, false).await;
    snapshot(state, false).await
}

#[tauri::command]
pub async fn get_snapshot(state: State<'_, AppState>) -> AppResult<Snapshot> {
    snapshot(&state, false).await
}

#[tauri::command]
pub async fn refresh_now(app: AppHandle, state: State<'_, AppState>) -> AppResult<Snapshot> {
    after_action(&app, &state).await
}

#[tauri::command]
pub async fn play(app: AppHandle, state: State<'_, AppState>, task_id: String) -> AppResult<Snapshot> {
    tracker::play(&state, &task_id).await?;
    if state.settings().open_in_browser_on_play {
        if let Some(t) = state.cached_task(&task_id).await {
            let _ = app.opener().open_url(t.url, None::<&str>);
        }
    }
    after_action(&app, &state).await
}

#[tauri::command]
pub async fn pause(app: AppHandle, state: State<'_, AppState>, reason: String) -> AppResult<Snapshot> {
    tracker::pause_current(&state, &reason).await?;
    after_action(&app, &state).await
}

#[tauri::command]
pub async fn stop(app: AppHandle, state: State<'_, AppState>, task_id: String) -> AppResult<Snapshot> {
    tracker::stop(&state, &task_id).await?;
    after_action(&app, &state).await
}

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> Settings {
    state.settings()
}

#[tauri::command]
pub async fn save_settings(app: AppHandle, state: State<'_, AppState>, mut settings: Settings) -> AppResult<Settings> {
    // Los tickets agregados a mano los administra el backend; no se toman del formulario.
    settings.extra_task_ids = state.settings().extra_task_ids;
    let team_changed = settings.team_id != state.settings().team_id;
    state.save_settings(settings)?;
    if team_changed {
        state.cache.lock().await.tasks.clear();
    }
    state.request_refresh();
    broadcast(&app, false).await;
    Ok(state.settings())
}

/// Valida el token contra la API antes de guardarlo en el llavero.
#[tauri::command]
pub async fn set_clickup_token(app: AppHandle, state: State<'_, AppState>, token: String) -> AppResult<User> {
    let token = token.trim().to_owned();
    if !token.starts_with("pk_") {
        return Err(AppError::invalid("El token personal de ClickUp empieza por «pk_»."));
    }
    let user = ClickUp::new(token.clone())?.me().await?;
    secrets::set(secrets::CLICKUP_TOKEN, &token)?;
    state.reset_client().await;
    state.cache.lock().await.user = Some(user.clone());
    state.request_refresh();
    broadcast(&app, true).await;
    Ok(user)
}

#[tauri::command]
pub async fn clear_clickup_token(app: AppHandle, state: State<'_, AppState>) -> AppResult<()> {
    secrets::delete(secrets::CLICKUP_TOKEN)?;
    state.reset_client().await;
    state.cache.lock().await.tasks.clear();
    broadcast(&app, false).await;
    Ok(())
}

#[tauri::command]
pub async fn list_teams(state: State<'_, AppState>) -> AppResult<Vec<Team>> {
    state.client().await?.teams().await
}

#[tauri::command]
pub fn set_smtp_password(password: String) -> AppResult<()> {
    if password.is_empty() {
        secrets::delete(secrets::SMTP_PASSWORD)
    } else {
        secrets::set(secrets::SMTP_PASSWORD, &password)
    }
}

#[tauri::command]
pub fn has_smtp_password() -> bool {
    secrets::get(secrets::SMTP_PASSWORD).ok().flatten().is_some()
}

#[tauri::command]
pub async fn send_test_email(state: State<'_, AppState>) -> AppResult<()> {
    mail::send_digest(&state).await
}

#[tauri::command]
pub async fn retry_failed(app: AppHandle, state: State<'_, AppState>) -> AppResult<Snapshot> {
    state.db.outbox_retry_failed(now_ms())?;
    after_action(&app, &state).await
}

#[tauri::command]
pub async fn discard_failed(app: AppHandle, state: State<'_, AppState>, id: i64) -> AppResult<Snapshot> {
    state.db.outbox_discard(id)?;
    after_action(&app, &state).await
}

#[tauri::command]
pub fn open_url(app: AppHandle, url: String) -> AppResult<()> {
    if !url.starts_with("https://app.clickup.com/") {
        return Err(AppError::invalid("Solo se abren enlaces de ClickUp."));
    }
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| AppError::invalid(e.to_string()))
}

#[tauri::command]
pub fn show_main(app: AppHandle, view: Option<String>) {
    crate::show_main(&app, view.as_deref());
}

#[tauri::command]
pub fn toggle_widget(app: AppHandle) {
    crate::toggle_widget(&app);
}

/// Ajusta la altura del widget (compacto / expandido con el motivo de pausa).
#[tauri::command]
pub fn resize_widget(app: AppHandle, height: f64) -> AppResult<()> {
    if let Some(w) = app.get_webview_window("widget") {
        w.set_size(tauri::LogicalSize::new(340.0, height.clamp(96.0, 420.0)))
            .map_err(|e| AppError::invalid(e.to_string()))?;
    }
    Ok(())
}

#[tauri::command]
pub fn get_autostart(app: AppHandle) -> bool {
    app.autolaunch().is_enabled().unwrap_or(false)
}

#[tauri::command]
pub fn set_autostart(app: AppHandle, enabled: bool) -> AppResult<()> {
    let al = app.autolaunch();
    let r = if enabled { al.enable() } else { al.disable() };
    r.map_err(|e| AppError::invalid(e.to_string()))
}
