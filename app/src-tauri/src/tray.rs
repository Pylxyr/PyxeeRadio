//! The tray icon and its menu. The app lives here while the window is closed: the window is really
//! destroyed (not hidden), because a hidden webview keeps its renderer processes and memory.

use crate::core_proc::{self, Phase};
use crate::state::Sh;
use crate::window;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use tauri::{image::Image, Emitter, Wry};

pub struct TrayItems {
    tray: TrayIcon<Wry>,
    start: MenuItem<Wry>,
    stop: MenuItem<Wry>,
    dark: CheckMenuItem<Wry>,
}

pub fn create(sh: &Sh) -> tauri::Result<()> {
    let app = &sh.app;
    let open = MenuItem::with_id(app, "open", "Open PyxeeRadio", true, None::<&str>)?;
    let start = MenuItem::with_id(app, "start", "Start", true, None::<&str>)?;
    let stop = MenuItem::with_id(app, "stop", "Stop", false, None::<&str>)?;
    let dark = CheckMenuItem::with_id(app, "dark", "Dark mode", true, false, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[&open, &PredefinedMenuItem::separator(app)?, &start, &stop, &PredefinedMenuItem::separator(app)?, &dark, &PredefinedMenuItem::separator(app)?, &quit],
    )?;
    let icon = Image::from_bytes(include_bytes!("../icons/tray.png"))?;
    let handler_sh = sh.clone();
    let tray = TrayIconBuilder::with_id("main")
        .icon(icon)
        .tooltip("PyxeeRadio")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(move |_app, event| {
            let sh = handler_sh.clone();
            match event.id().as_ref() {
                "open" => window::show_window(&sh),
                "start" => {
                    tauri::async_runtime::spawn(async move {
                        core_proc::start_bot(&sh).await;
                    });
                }
                "stop" => {
                    tauri::async_runtime::spawn(async move { core_proc::stop_bot(&sh).await });
                }
                "dark" => toggle_theme(&sh),
                "quit" => {
                    tauri::async_runtime::spawn(async move { window::quit_app(&sh).await });
                }
                _ => {}
            }
        })
        .on_tray_icon_event({
            let sh = sh.clone();
            move |_tray, event| {
                if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                    window::show_window(&sh);
                }
            }
        })
        .build(app)?;
    *sh.tray.lock().unwrap() = Some(TrayItems { tray, start, stop, dark });
    refresh(sh);
    Ok(())
}

/// Whether the window is dark right now: what was asked for, or (for "system") what it looked like last.
pub fn effective_dark(sh: &Sh) -> bool {
    let prefs = sh.prefs_snapshot();
    match prefs.theme.as_str() {
        "dark" => true,
        "light" => false,
        _ => prefs.window_dark,
    }
}

pub fn refresh(sh: &Sh) {
    let guard = sh.tray.lock().unwrap();
    let Some(items) = guard.as_ref() else { return };
    let phase = sh.core.phase();
    let running = matches!(phase, Phase::Running | Phase::Starting);
    let _ = items.start.set_enabled(!sh.core.is_running_process());
    let _ = items.stop.set_enabled(running);
    let _ = items.dark.set_checked(effective_dark(sh));
    let _ = items.tray.set_tooltip(Some(format!("PyxeeRadio - {}", phase.label())));
}

/// The tray's Dark mode switch: light or dark explicitly (the Appearance setting is where "System" lives).
pub fn toggle_theme(sh: &Sh) {
    let next = if effective_dark(sh) { "light" } else { "dark" };
    let prefs = sh.update_prefs(|p| p.theme = next.to_string());
    window::apply_theme(sh);
    let _ = sh.app.emit("prefs:changed", &prefs);
    refresh(sh);
}
