//! TicketPilot: cronómetro flotante y control de tickets de ClickUp.
//!
//! Arquitectura:
//! - `tracker`  → Play/Pausa/Stop como transacciones locales + outbox.
//! - `sync`     → bucle de fondo que replica el outbox y recarga los tickets.
//! - `state`    → caché en memoria y la foto (snapshot) que reciben las ventanas.
//! - `mail`     → resumen diario por SMTP.
//! - Ventanas: `widget` (siempre visible, siempre encima) y `main` (panel).

mod clickup;
mod commands;
mod config;
mod db;
mod error;
mod fields;
mod mail;
mod secrets;
mod state;
mod sync;
mod tracker;

use std::sync::atomic::AtomicBool;
use std::sync::RwLock;

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, WindowEvent};
use tokio::sync::{Mutex, Notify};

use crate::config::Settings;
use crate::db::Db;
use crate::state::{AppState, Cache};

pub const EVENT_NAVIGATE: &str = "navigate";

/// Muestra el panel principal y, opcionalmente, lo lleva a una vista.
pub fn show_main(app: &AppHandle, view: Option<&str>) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
        if let Some(v) = view {
            let _ = w.emit(EVENT_NAVIGATE, v);
        }
    }
}

pub fn toggle_widget(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("widget") {
        if w.is_visible().unwrap_or(false) {
            let _ = w.hide();
        } else {
            let _ = w.show();
        }
    }
}

fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Abrir panel", true, None::<&str>)?;
    let widget = MenuItem::with_id(app, "widget", "Mostrar u ocultar cronómetro", true, None::<&str>)?;
    let sync = MenuItem::with_id(app, "sync", "Sincronizar ahora", true, None::<&str>)?;
    let sep = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", "Salir de TicketPilot", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &widget, &sync, &sep, &quit])?;

    let mut tray = TrayIconBuilder::with_id("main")
        .tooltip("TicketPilot")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, ev| match ev.id().as_ref() {
            "open" => show_main(app, None),
            "widget" => toggle_widget(app),
            "sync" => app.state::<AppState>().request_refresh(),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, ev| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = ev {
                show_main(tray.app_handle(), None);
            }
        });
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.build(app)?;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    tauri::Builder::default()
        // Una segunda ejecución solo trae al frente la que ya corre.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| show_main(app, None)))
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--minimized"]),
        ))
        .setup(|app| {
            let config_dir = app.path().app_config_dir()?;
            let data_dir = app.path().app_data_dir()?;
            let db = Db::open(&data_dir.join("ticketpilot.sqlite3"))?;
            let settings = Settings::load(&config_dir);

            app.manage(AppState {
                db,
                config_dir,
                settings: RwLock::new(settings),
                cache: Mutex::new(Cache::default()),
                wake: Notify::new(),
                dirty: AtomicBool::new(true),
            });

            build_tray(app.handle())?;

            // Primera ejecución (sin token): abrir el panel para configurarlo.
            let configured = secrets::get(secrets::CLICKUP_TOKEN).ok().flatten().is_some();
            let minimized = std::env::args().any(|a| a == "--minimized");
            if !configured || !minimized {
                show_main(app.handle(), (!configured).then_some("settings"));
            }

            sync::spawn(app.handle().clone());
            Ok(())
        })
        // Cerrar una ventana la oculta: la app sigue en la bandeja del sistema.
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_snapshot,
            commands::refresh_now,
            commands::play,
            commands::pause,
            commands::stop,
            commands::get_settings,
            commands::save_settings,
            commands::set_clickup_token,
            commands::clear_clickup_token,
            commands::list_teams,
            commands::set_smtp_password,
            commands::has_smtp_password,
            commands::send_test_email,
            commands::retry_failed,
            commands::discard_failed,
            commands::open_url,
            commands::show_main,
            commands::toggle_widget,
            commands::resize_widget,
            commands::get_autostart,
            commands::set_autostart,
            fields::get_fill_form,
            fields::save_fields,
            fields::list_required_fields,
            fields::set_field_applies,
        ])
        .run(tauri::generate_context!())
        .expect("no se pudo iniciar TicketPilot");
}
