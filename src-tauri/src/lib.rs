mod api_server;
mod audit_log;
mod autostart;
mod backup;
mod capability;
mod cdp;
mod commands;
mod crypto;
mod db;
mod error;
mod fingerprint;
mod fingerprint_report;
mod maintenance;
mod models;
mod package;
mod paths;
mod process;
mod proxy_check;
mod services;
mod state;

use std::sync::OnceLock;

use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager, WindowEvent,
};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use state::SharedState;

static LOG_GUARD: OnceLock<tracing_appender::non_blocking::WorkerGuard> = OnceLock::new();

fn init_logging(data_dir: &std::path::Path) -> Option<std::path::PathBuf> {
    let logs = paths::logs_dir(data_dir);
    let _ = std::fs::create_dir_all(&logs);
    let file_appender = tracing_appender::rolling::daily(&logs, "finenv.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);
    let _ = LOG_GUARD.set(guard);
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let file_layer = tracing_subscriber::fmt::layer()
        .json()
        .with_writer(non_blocking)
        .with_target(true);
    let console_layer = tracing_subscriber::fmt::layer().with_target(true);
    let result = tracing_subscriber::registry()
        .with(filter)
        .with(file_layer)
        .with(console_layer)
        .try_init();
    if let Err(e) = result {
        eprintln!("tracing init failed: {e}");
    }
    Some(logs)
}

fn setup_tray(app: &tauri::App) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show", "显示 FinEnv", true, None::<&str>)?;
    let recycle = MenuItem::with_id(app, "recycle", "一键回收全部浏览器", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &recycle, &quit])?;
    let mut builder = TrayIconBuilder::with_id("finenv-tray")
        .tooltip("FinEnv 指纹环境")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
            "recycle" => {
                let state = app.state::<SharedState>().inner().clone();
                tauri::async_runtime::spawn(async move {
                    let actor = state
                        .current_user()
                        .map(|u| u.username)
                        .unwrap_or_else(|| "tray".into());
                    let _ = state.browsers.force_recycle_all(&actor).await;
                });
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                if let Some(window) = tray.app_handle().get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    Ok(())
}

fn setup_shortcut(app: &tauri::App, state: &SharedState) {
    let shortcut = state.settings().global_shortcut;
    if shortcut.trim().is_empty() {
        return;
    }
    let result =
        app.global_shortcut()
            .on_shortcut(shortcut.as_str(), move |app, _shortcut, event| {
                if event.state() == ShortcutState::Pressed {
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
            });
    if let Err(e) = result {
        tracing::warn!(shortcut = %shortcut, error = %e, "global shortcut registration failed");
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let data_dir = paths::data_dir();
    let _ = std::fs::create_dir_all(&data_dir);
    init_logging(&data_dir);

    let state = match state::AppState::initialize() {
        Ok(state) => state,
        Err(e) => {
            eprintln!("FinEnv 初始化失败: {}", e.message());
            std::process::exit(1);
        }
    };
    let setup_state = state.clone();

    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .manage(state)
        .setup(move |app| {
            let handle = app.handle().clone();
            setup_state.attach_app(handle.clone());
            if let Err(e) = setup_tray(app) {
                tracing::warn!(error = %e, "tray setup failed");
            }
            setup_shortcut(app, &setup_state);
            let state = setup_state.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = maintenance::upgrade_backup(&state) {
                    tracing::warn!(error = %e, "upgrade backup failed");
                }
                match state.browsers.restore_sessions().await {
                    Ok(count) if count > 0 => {
                        tracing::info!(count, "restored browser sessions after panel restart");
                    }
                    Ok(_) => {}
                    Err(e) => tracing::warn!(error = %e, "session restore failed"),
                }
                state.browsers.start_monitor();
                if state.settings().api_enabled {
                    match state.api.start(state.clone()).await {
                        Ok(status) => tracing::info!(url = %status.base_url, "API server ready"),
                        Err(e) => {
                            tracing::error!(error = %e, "API server failed to start");
                            use tauri::Emitter;
                            let _ = handle
                                .emit("api://error", serde_json::json!({ "message": e.message() }));
                        }
                    }
                }
            });
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                let state = window.state::<SharedState>();
                if state.settings().close_to_tray {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::auth_status,
            commands::auth_login,
            commands::auth_logout,
            commands::auth_change_password,
            commands::user_list,
            commands::user_create,
            commands::user_update,
            commands::user_delete,
            commands::permission_matrix,
            commands::env_list,
            commands::env_get,
            commands::env_check_ip,
            commands::env_create,
            commands::env_update,
            commands::env_delete,
            commands::env_bulk_delete,
            commands::env_clone,
            commands::env_export,
            commands::env_import,
            commands::env_bulk_import,
            commands::env_projects,
            commands::env_tags,
            commands::group_list,
            commands::group_create,
            commands::group_update,
            commands::group_delete,
            commands::env_assign_group,
            commands::account_list,
            commands::account_create,
            commands::account_update,
            commands::account_delete,
            commands::account_reveal,
            commands::account_import,
            commands::account_import_file,
            commands::account_export,
            commands::fingerprint_template,
            commands::fingerprint_validate,
            commands::fingerprint_update,
            commands::fingerprint_detect,
            commands::fingerprint_reports,
            commands::snapshot_list,
            commands::snapshot_create,
            commands::snapshot_restore,
            commands::snapshot_delete,
            commands::proxy_list,
            commands::proxy_create,
            commands::proxy_update,
            commands::proxy_delete,
            commands::proxy_check,
            commands::proxy_assign,
            commands::proxy_align,
            commands::browser_open,
            commands::browser_close,
            commands::browser_status,
            commands::browser_list_running,
            commands::browser_screenshot,
            commands::browser_cookies_get,
            commands::browser_cookies_set,
            commands::browser_execute,
            commands::browser_navigate,
            commands::browser_cdp_info,
            commands::browser_recording_start,
            commands::browser_recording_stop,
            commands::browser_har_start,
            commands::browser_har_stop,
            commands::browser_force_recycle_all,
            commands::browser_switch_proxy,
            commands::resource_snapshot,
            commands::hosts_list,
            commands::hosts_set,
            commands::mock_list,
            commands::mock_set,
            commands::extension_list,
            commands::extension_add,
            commands::extension_delete,
            commands::extension_assign,
            commands::extension_unassign,
            commands::env_extensions,
            commands::captcha_list,
            commands::captcha_handle,
            commands::captcha_capture,
            commands::read_artifact,
            commands::settings_get,
            commands::settings_update,
            commands::api_status,
            commands::api_restart,
            commands::api_key_list,
            commands::api_key_create,
            commands::api_key_revoke,
            commands::theme_get,
            commands::theme_save,
            commands::theme_export,
            commands::theme_import,
            commands::audit_list,
            commands::audit_verify,
            commands::audit_export,
            commands::dashboard,
            commands::app_info,
            commands::chromium_detect,
            commands::update_check,
            commands::backup_create,
            commands::backup_restore,
            commands::backup_list,
            commands::sync_test,
            commands::sync_push,
            commands::sync_pull,
        ]);

    if let Err(e) = builder.run(tauri::generate_context!()) {
        tracing::error!(error = %e, "FinEnv exited with error");
        eprintln!("FinEnv exited with error: {e}");
    }
}
