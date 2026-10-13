//! PyxeeRadio desktop shell.
//!
//! Owns three things: the window and tray, the bot process (a hidden child that speaks JSON lines on
//! stdin/stdout; the protocol is documented in twitch_radio/headless.py), and the settings files the
//! Settings tab edits. All UI lives in app/src (Svelte); it only sees the commands in commands.rs.

mod commands;
mod config;
mod core_proc;
mod fsutil;
mod logs;
mod paths;
mod prefs;
mod state;
mod tray;
mod updates;
mod validate;
mod window;

use state::{Shared, Sh};
use std::sync::atomic::Ordering;
use tauri::{Manager, RunEvent};

pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(sh) = app.try_state::<Sh>() {
                window::show_window(&sh);
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::LaunchAgent, Some(vec!["--hidden"])))
        .setup(|app| {
            let sh = Shared::new(app.handle().clone());
            app.manage(sh.clone());
            // Launched at login: start with only the tray icon; the window is built when it is first opened.
            let start_hidden = std::env::args().any(|arg| arg == "--hidden");
            tray::create(&sh)?;
            if !start_hidden {
                window::create_window(&sh);
            }
            sh.logs.gui("INFO", &format!("PyxeeRadio {} - data folder: {}", config::app_version(&sh), sh.paths.home.display()));
            if sh.prefs_snapshot().start_bot_on_launch {
                let sh = sh.clone();
                tauri::async_runtime::spawn(async move {
                    core_proc::start_bot(&sh).await;
                });
            }
            config::schedule_startup_checks(&sh);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::bot_status,
            commands::bot_start,
            commands::bot_stop,
            commands::bot_restart,
            commands::bot_command,
            commands::bot_request_song,
            commands::bot_search_songs,
            commands::radio_get_lookahead,
            commands::radio_set_lookahead,
            commands::logs_get,
            commands::logs_clear,
            commands::logs_export,
            commands::config_preflight,
            commands::config_get_env,
            commands::config_save_env,
            commands::config_get_live,
            commands::config_save_live,
            commands::prefs_get,
            commands::prefs_set,
            commands::set_appearance,
            commands::ui_ready,
            commands::hide_to_tray,
            commands::close_decision,
            commands::update_app,
            commands::update_app_last,
            commands::update_ytdlp_check,
            commands::update_ytdlp_install,
            commands::update_ytdlp_rollback,
            commands::open_external,
            commands::open_path,
            commands::import_cookies,
            commands::app_info,
        ])
        .build(tauri::generate_context!())
        .expect("error while building PyxeeRadio");

    app.run(|handle, event| match event {
        // Closing the window (hide to tray) must not end the app: it lives in the tray until Quit.
        RunEvent::ExitRequested { api, code, .. } => {
            if code.is_none() {
                if let Some(sh) = handle.try_state::<Sh>() {
                    if !sh.quitting.load(Ordering::SeqCst) {
                        api.prevent_exit();
                    }
                }
            }
        }
        // However the app ends, no ffmpeg or lookup worker may be left behind.
        RunEvent::Exit => {
            if let Some(sh) = handle.try_state::<Sh>() {
                core_proc::shutdown_blocking(&sh);
            }
        }
        _ => {}
    });
}
