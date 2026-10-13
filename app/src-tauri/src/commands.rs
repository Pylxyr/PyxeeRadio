//! The only things the page can ask the shell to do. Each one checks what it is given: the page is a
//! local file and trusted to draw, but not to be the last word on what gets written, opened or run.

use crate::logs::{export_line, file_stamp};
use crate::state::Sh;
use crate::validate::{clean_query, is_hex_color};
use crate::{config, core_proc, paths, tray, window};
use serde_json::{json, Value};
use std::time::Duration;
use tauri::{Manager, State};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;

// ---------------------------------------------------------------- the bot ---

#[tauri::command]
pub fn bot_status(sh: State<'_, Sh>) -> Value {
    json!({ "status": core_proc::status(&sh), "state": sh.core.last_state() })
}

#[tauri::command]
pub async fn bot_start(sh: State<'_, Sh>) -> Result<Value, ()> {
    Ok(core_proc::start_bot(&sh).await)
}

#[tauri::command]
pub async fn bot_stop(sh: State<'_, Sh>) -> Result<Value, ()> {
    core_proc::stop_bot(&sh).await;
    Ok(core_proc::status(&sh))
}

#[tauri::command]
pub async fn bot_restart(sh: State<'_, Sh>) -> Result<Value, ()> {
    Ok(core_proc::restart_bot(&sh).await)
}

#[tauri::command]
pub async fn bot_command(sh: State<'_, Sh>, name: String) -> Result<Value, ()> {
    if !["skip", "pause", "resume", "clear_queue"].contains(&name.as_str()) {
        return Ok(json!({ "ok": false, "error": "Unknown command." }));
    }
    // A skip may wait a few seconds for the next song to be ready (see RadioPlayer.skip), so it gets more time.
    let timeout = if name == "skip" { Duration::from_secs(15) } else { Duration::from_secs(4) };
    Ok(core_proc::send_command(&sh, &name, json!({}), timeout).await)
}

/// A song typed into the dashboard's search bar: queued for the streamer, answered when found.
#[tauri::command]
pub async fn bot_request_song(sh: State<'_, Sh>, text: Value) -> Result<Value, ()> {
    let Some(query) = clean_query(&text) else { return Ok(json!({ "ok": false, "error": "Type a song name or paste a link." })) };
    Ok(core_proc::send_command(&sh, "request", json!({ "query": query }), Duration::from_secs(90)).await)
}

/// The search bar's first step: a few matches to choose from; choosing one is a `bot_request_song`.
#[tauri::command]
pub async fn bot_search_songs(sh: State<'_, Sh>, text: Value) -> Result<Value, ()> {
    let Some(query) = clean_query(&text) else { return Ok(json!({ "ok": false, "error": "Type a song name to search for." })) };
    Ok(core_proc::send_command(&sh, "search", json!({ "query": query }), Duration::from_secs(40)).await)
}

#[tauri::command]
pub async fn radio_get_lookahead(sh: State<'_, Sh>) -> Result<Value, ()> {
    Ok(config::get_lookahead(&sh).await)
}

#[tauri::command]
pub async fn radio_set_lookahead(sh: State<'_, Sh>, value: Value) -> Result<Value, ()> {
    Ok(config::set_lookahead(&sh, &value).await)
}

// ------------------------------------------------------------------- logs ---

#[tauri::command]
pub fn logs_get(sh: State<'_, Sh>) -> Value {
    json!(sh.logs.all())
}

#[tauri::command]
pub fn logs_clear(sh: State<'_, Sh>) -> bool {
    sh.logs.clear();
    true
}

#[tauri::command]
pub async fn logs_export(sh: State<'_, Sh>) -> Result<Value, ()> {
    let now = crate::updates::now_ms() as f64 / 1000.0;
    let dir = sh.app.path().document_dir().unwrap_or_else(|_| sh.paths.home.clone());
    let app = sh.app.clone();
    let picked = tauri::async_runtime::spawn_blocking(move || {
        app.dialog()
            .file()
            .set_title("Export logs")
            .set_directory(dir)
            .set_file_name(format!("pyxee-radio-{}.log", file_stamp(now)))
            .add_filter("Log file", &["log", "txt"])
            .blocking_save_file()
    })
    .await
    .ok()
    .flatten()
    .and_then(|p| p.into_path().ok());
    let Some(path) = picked else { return Ok(json!({ "ok": false, "canceled": true })) };
    let text = sh.logs.all().iter().map(export_line).collect::<Vec<_>>().join("\n") + "\n";
    Ok(match std::fs::write(&path, text) {
        Ok(()) => json!({ "ok": true, "filePath": path.to_string_lossy() }),
        Err(error) => json!({ "ok": false, "error": error.to_string() }),
    })
}

// ----------------------------------------------------------- configuration ---

#[tauri::command]
pub async fn config_preflight(sh: State<'_, Sh>, force: Option<bool>) -> Result<Value, ()> {
    Ok(config::preflight(&sh, force != Some(false)).await)
}

#[tauri::command]
pub async fn config_get_env(sh: State<'_, Sh>) -> Result<Value, ()> {
    Ok(config::get_env(&sh).await)
}

#[tauri::command]
pub async fn config_save_env(sh: State<'_, Sh>, payload: Value) -> Result<Value, ()> {
    Ok(config::save_env(&sh, &payload).await)
}

#[tauri::command]
pub async fn config_get_live(sh: State<'_, Sh>) -> Result<Value, ()> {
    Ok(config::get_live(&sh).await)
}

#[tauri::command]
pub async fn config_save_live(sh: State<'_, Sh>, payload: Value) -> Result<Value, ()> {
    Ok(config::save_live(&sh, &payload).await)
}

#[tauri::command]
pub fn prefs_get(sh: State<'_, Sh>) -> Value {
    json!(sh.prefs_snapshot())
}

#[tauri::command]
pub fn prefs_set(sh: State<'_, Sh>, patch: Value) -> Value {
    let before = sh.prefs_snapshot();
    let prefs = sh.update_prefs(|p| p.apply_patch(&patch));
    window::apply_theme(&sh);
    tray::refresh(&sh);
    if sh.paths.packaged && prefs.launch_at_login != before.launch_at_login {
        use tauri_plugin_autostart::ManagerExt;
        let manager = sh.app.autolaunch();
        let result = if prefs.launch_at_login { manager.enable() } else { manager.disable() };
        if let Err(error) = result {
            sh.logs.gui("WARNING", &format!("Couldn't change the start-at-login setting: {error}"));
        }
    }
    json!(prefs)
}

/// What the page looks like now (its background, and whether that is dark), remembered so the next
/// window starts in the right colour and the tray's Dark mode switch shows the right state.
#[tauri::command]
pub fn set_appearance(sh: State<'_, Sh>, bg: String, dark: bool) {
    if !is_hex_color(&bg) {
        return;
    }
    let before = sh.prefs_snapshot();
    if before.window_bg != bg || before.window_dark != dark {
        sh.update_prefs(|p| {
            p.window_bg = bg;
            p.window_dark = dark;
        });
        tray::refresh(&sh);
    }
}

// ----------------------------------------------------------------- window ---

/// The page has painted: show the window (it is created hidden so it never flashes the wrong colour).
#[tauri::command]
pub fn ui_ready(sh: State<'_, Sh>) {
    if let Some(window) = window::window(&sh) {
        window::present(&sh, &window);
    }
}

#[tauri::command]
pub fn hide_to_tray(sh: State<'_, Sh>) {
    window::hide_to_tray(&sh);
}

/// The answer to the "still running" question the page asked on `close:requested`.
#[tauri::command]
pub async fn close_decision(sh: State<'_, Sh>, action: String, remember: bool) -> Result<(), ()> {
    if action != "tray" && action != "quit" {
        return Ok(()); // cancel
    }
    if remember {
        sh.update_prefs(|p| {
            p.ask_on_close = false;
            p.close_action = action.clone();
        });
    }
    if action == "quit" {
        window::quit_app(&sh).await;
    } else {
        window::hide_to_tray(&sh);
    }
    Ok(())
}

// ---------------------------------------------------------------- updates ---

#[tauri::command]
pub async fn update_app(sh: State<'_, Sh>) -> Result<Value, ()> {
    sh.update_prefs(|p| p.last_update_check = crate::updates::now_ms());
    Ok(config::check_app_update_now(&sh).await)
}

#[tauri::command]
pub fn update_app_last(sh: State<'_, Sh>) -> Value {
    json!(sh.app_update.lock().unwrap().clone())
}

#[tauri::command]
pub async fn update_ytdlp_check(sh: State<'_, Sh>) -> Result<Value, ()> {
    Ok(config::ytdlp_check(&sh).await)
}

#[tauri::command]
pub async fn update_ytdlp_install(sh: State<'_, Sh>) -> Result<Value, ()> {
    Ok(config::ytdlp_change(&sh, "--ytdlp-install").await)
}

#[tauri::command]
pub async fn update_ytdlp_rollback(sh: State<'_, Sh>) -> Result<Value, ()> {
    Ok(config::ytdlp_change(&sh, "--ytdlp-rollback").await)
}

// ------------------------------------------------------------------ shell ---

/// Only web links, and only to open them in the browser.
pub fn open_external_safe(sh: &Sh, url: &str) -> bool {
    match url::Url::parse(url.trim()) {
        Ok(parsed) if matches!(parsed.scheme(), "http" | "https") => sh.app.opener().open_url(parsed.as_str(), None::<&str>).is_ok(),
        _ => false,
    }
}

#[tauri::command]
pub fn open_external(sh: State<'_, Sh>, url: String) -> bool {
    open_external_safe(&sh, &url)
}

#[tauri::command]
pub fn open_path(sh: State<'_, Sh>, which: String) -> bool {
    let home = &sh.paths.home;
    let target = match which.as_str() {
        "home" => home.clone(),
        "data" => home.join("data"),
        "logs" => home.join("logs"),
        "env" => home.join(".env"),
        _ => return false,
    };
    if which == "env" {
        return sh.app.opener().reveal_item_in_dir(&target).is_ok();
    }
    let _ = std::fs::create_dir_all(&target);
    sh.app.opener().open_path(target.to_string_lossy(), None::<&str>).is_ok()
}

/// yt-dlp rewrites its cookie jar while it runs, so the file has to sit in a folder the bot can write
/// to. Copy the picked export into the data folder and hand back the relative path the bot's config expects.
#[tauri::command]
pub async fn import_cookies(sh: State<'_, Sh>) -> Result<Value, ()> {
    let app = sh.app.clone();
    let picked = tauri::async_runtime::spawn_blocking(move || {
        app.dialog()
            .file()
            .set_title("Choose your exported cookies.txt")
            .add_filter("Cookies (Netscape format)", &["txt"])
            .blocking_pick_file()
    })
    .await
    .ok()
    .flatten()
    .and_then(|p| p.into_path().ok());
    let Some(source) = picked else { return Ok(Value::Null) };
    let data = sh.paths.home.join("data");
    let target = data.join("cookies.txt");
    if std::fs::create_dir_all(&data).and_then(|()| std::fs::copy(&source, &target).map(|_| ())).is_err() {
        sh.logs.gui("ERROR", "Couldn't import the cookies file.");
        return Ok(Value::Null);
    }
    // Session cookies are as good as a password: owner-only where the OS has file modes.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o600));
    }
    sh.logs.gui("INFO", "Imported cookies.txt into the data folder.");
    Ok(json!("data/cookies.txt"))
}

#[tauri::command]
pub fn app_info(sh: State<'_, Sh>) -> Value {
    json!({
        "version": config::app_version(&sh),
        "home": sh.paths.home.to_string_lossy(),
        "packaged": sh.paths.packaged,
        "platform": paths::platform(),
        "runtime": "tauri",
    })
}
