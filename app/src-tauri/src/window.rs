//! The window: created when it is wanted and destroyed when it is not.

use crate::core_proc::{self, Phase};
use crate::state::Sh;
use crate::validate::is_hex_color;
use std::sync::atomic::Ordering;
use std::time::Duration;
use tauri::window::Color;
use tauri::{Emitter, Manager, Theme, WebviewUrl, WebviewWindow, WebviewWindowBuilder, WindowEvent};

pub const LABEL: &str = "main";

fn theme_of(preference: &str) -> Option<Theme> {
    match preference {
        "light" => Some(Theme::Light),
        "dark" => Some(Theme::Dark),
        _ => None, // follow the system
    }
}

fn color_of(hex: &str) -> Color {
    if !is_hex_color(hex) {
        return Color(11, 11, 16, 255);
    }
    let channel = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).unwrap_or(0);
    Color(channel(1), channel(3), channel(5), 255)
}

pub fn window(sh: &Sh) -> Option<WebviewWindow> {
    sh.app.get_webview_window(LABEL)
}

/// Makes the native frame (and, for "System", the page's prefers-color-scheme) follow the Appearance setting.
pub fn apply_theme(sh: &Sh) {
    if let Some(window) = window(sh) {
        let _ = window.set_theme(theme_of(&sh.prefs_snapshot().theme));
    }
}

pub fn create_window(sh: &Sh) {
    if window(sh).is_some() {
        return;
    }
    let prefs = sh.prefs_snapshot();
    let built = WebviewWindowBuilder::new(&sh.app, LABEL, WebviewUrl::App("index.html".into()))
        .title("PyxeeRadio")
        .inner_size(1220.0, 780.0)
        .min_inner_size(980.0, 640.0)
        .visible(false) // shown by `ui_ready`, once the page has painted
        .background_color(color_of(&prefs.window_bg))
        .theme(theme_of(&prefs.theme))
        // Windows gets the app's own title bar (it holds the app's controls anyway); elsewhere the system frame is kept.
        .decorations(!cfg!(windows))
        .build();
    let window = match built {
        Ok(window) => window,
        Err(error) => {
            sh.logs.gui("ERROR", &format!("Couldn't open the window: {error}"));
            return;
        }
    };
    let events_sh = sh.clone();
    window.on_window_event(move |event| match event {
        WindowEvent::CloseRequested { api, .. } => on_close_requested(&events_sh, api),
        WindowEvent::Focused(_) | WindowEvent::Resized(_) => core_proc::update_watching(&events_sh, false),
        WindowEvent::Destroyed => {
            let sh = events_sh.clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(Duration::from_millis(100)).await;
                core_proc::update_watching(&sh, false);
            });
        }
        _ => {}
    });
    // If the page never reports in, the window must still appear.
    let sh = sh.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_secs(4)).await;
        if let Some(window) = window_by_sh(&sh) {
            if !window.is_visible().unwrap_or(true) {
                present(&sh, &window);
            }
        }
    });
}

fn window_by_sh(sh: &Sh) -> Option<WebviewWindow> {
    window(sh)
}

pub fn present(sh: &Sh, window: &WebviewWindow) {
    if window.is_minimized().unwrap_or(false) {
        let _ = window.unminimize();
    }
    let _ = window.show();
    let _ = window.set_focus();
    core_proc::update_watching(sh, false);
}

pub fn show_window(sh: &Sh) {
    match window(sh) {
        Some(window) => present(sh, &window),
        None => create_window(sh),
    }
}

fn on_close_requested(sh: &Sh, api: &tauri::CloseRequestApi) {
    if sh.quitting.load(Ordering::SeqCst) {
        return;
    }
    let running = sh.core.is_running_process() && sh.core.phase() != Phase::Stopped;
    if !running {
        sh.quitting.store(true, Ordering::SeqCst); // nothing to protect: closing the window quits the app
        return;
    }
    api.prevent_close();
    let prefs = sh.prefs_snapshot();
    if prefs.ask_on_close {
        // The page asks (a dialog of the app's own, with its "don't ask again" box) and answers with `close_decision`.
        let _ = sh.app.emit_to(LABEL, "close:requested", ());
        return;
    }
    let sh = sh.clone();
    tauri::async_runtime::spawn(async move {
        if prefs.close_action == "quit" {
            quit_app(&sh).await;
        } else {
            hide_to_tray(&sh);
        }
    });
}

/// "Keep running in tray": the window is destroyed, not hidden. A hidden window keeps its webview
/// processes (well over 100 MB); closing it for real frees them, and the page rebuilds its whole state
/// from the shell when it is opened again.
pub fn hide_to_tray(sh: &Sh) {
    if let Some(window) = window(sh) {
        let _ = window.destroy();
    }
}

/// Stops the bot cleanly (it saves its queue and tokens), then really quits.
pub async fn quit_app(sh: &Sh) {
    sh.quitting.store(true, Ordering::SeqCst);
    core_proc::stop_bot(sh).await;
    sh.app.exit(0);
}
