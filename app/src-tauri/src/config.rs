//! Configuration: the setup check, the settings in .env (read and written by the core itself, so there
//! is one parser for the file), the live limits and the radio lookahead the bot watches on disk, and
//! the yt-dlp and app update checks.

use crate::core_proc::{self, hide_console};
use crate::fsutil::{atomic_write, read_json_object};
use crate::state::Sh;
use crate::updates::{self, check_for_app_update, github_fetch, now_ms, UPDATE_REPO};
use crate::validate::{normalize_lookahead, parse_int_value};
use serde_json::{json, Map, Value};
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};
use tauri::Emitter;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

/// The .env keys the page may read and write. Anything else is refused.
pub const ENV_KEYS: [&str; 15] = [
    "OVERLAY_DELAY_SECONDS",
    "TWITCH_CLIENT_ID",
    "TWITCH_CLIENT_SECRET",
    "TWITCH_BOT_ID",
    "TWITCH_OWNER_ID",
    "AUDIO_BITRATE_KBPS",
    "PAUSE_QUEUE_WHEN_NO_LISTENERS",
    "TWITCH_NOWPLAYING_PORT",
    "YTDLP_COOKIES_FILE",
    "YTDLP_CONCURRENCY",
    "YTDLP_WORKER_IDLE_SECONDS",
    "YTDLP_EXTRACT_TIMEOUT_SECONDS",
    "YTDLP_CACHE_TTL_SECONDS",
    "LOG_LEVEL",
    "LOG_TO_FILE",
];

const PREFLIGHT_FRESH: Duration = Duration::from_secs(120);

pub struct RunResult {
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
    pub error: Option<String>,
}

/// One-shot run of the core (preflight, env read/write, yt-dlp update checks).
pub async fn run_core(sh: &Sh, extra: &[&str], input: Option<String>, timeout: Duration) -> RunResult {
    let (program, mut args) = sh.paths.core_command();
    args.extend(extra.iter().map(|a| a.to_string()));
    let mut command = Command::new(program);
    command
        .args(&args)
        .envs(sh.paths.core_env())
        .current_dir(sh.paths.core_cwd())
        .stdin(if input.is_some() { std::process::Stdio::piped() } else { std::process::Stdio::null() })
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);
    hide_console(&mut command);
    let failed = |error: String| RunResult { code: 1, stdout: String::new(), stderr: String::new(), error: Some(error) };
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => return failed(error.to_string()),
    };
    if let (Some(text), Some(mut stdin)) = (input, child.stdin.take()) {
        let _ = stdin.write_all(text.as_bytes()).await;
        drop(stdin);
    }
    match tokio::time::timeout(timeout, child.wait_with_output()).await {
        Ok(Ok(output)) => RunResult {
            code: output.status.code().unwrap_or(1),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            error: None,
        },
        Ok(Err(error)) => failed(error.to_string()),
        Err(_) => failed("timed out".into()),
    }
}

/// The last line of `text` that is a JSON value (warnings may precede a report).
pub fn last_json_line(text: &str) -> Option<Value> {
    text.trim().lines().rev().find_map(|line| serde_json::from_str(line.trim()).ok())
}

fn tail(text: &str, max: usize) -> String {
    let text = text.trim();
    let skip = text.chars().count().saturating_sub(max);
    text.chars().skip(skip).collect()
}

/// Runs the core's preflight (it starts a short-lived process, so nothing polls it). A page that is
/// just being reopened gets the cached report; callers that know something changed pass `force`.
pub async fn preflight(sh: &Sh, force: bool) -> Value {
    if !force {
        if let Some((report, at)) = sh.preflight.lock().unwrap().as_ref() {
            if at.elapsed() < PREFLIGHT_FRESH {
                return report.clone();
            }
        }
    }
    let result = run_core(sh, &["--preflight"], None, Duration::from_secs(30)).await;
    let report = last_json_line(&result.stdout).unwrap_or_else(|| {
        json!({
            "ok": false,
            "config_ok": false,
            "config_error": "The bot core could not be started to check the setup.",
            "detail": tail(&if result.stderr.is_empty() { result.error.clone().unwrap_or_default() } else { result.stderr.clone() }, 600),
        })
    });
    *sh.preflight.lock().unwrap() = Some((report.clone(), Instant::now()));
    report
}

fn last_preflight(sh: &Sh) -> Option<Value> {
    sh.preflight.lock().unwrap().as_ref().map(|(report, _)| report.clone())
}

/// (bounds, defaults) of the live limits, from the last preflight report.
fn tunable_spec(sh: &Sh) -> (Map<String, Value>, Map<String, Value>) {
    let report = last_preflight(sh).unwrap_or(Value::Null);
    let pick = |key: &str| report.pointer(&format!("/tunables/{key}")).and_then(Value::as_object).cloned().unwrap_or_default();
    (pick("bounds"), pick("defaults"))
}

struct LiveFiles {
    tunables: PathBuf,
    toggles: PathBuf,
    lookahead: PathBuf,
}

async fn live_files(sh: &Sh) -> LiveFiles {
    let have = last_preflight(sh).is_some_and(|r| r.get("files").is_some() && r.get("tunables").is_some());
    if !have {
        preflight(sh, true).await;
    }
    let report = last_preflight(sh).unwrap_or(Value::Null);
    let file = |key: &str, default: &str| {
        report.pointer(&format!("/files/{key}")).and_then(Value::as_str).map(PathBuf::from).unwrap_or_else(|| sh.paths.home.join("data").join(default))
    };
    LiveFiles { tunables: file("tunables", "tunables.json"), toggles: file("toggles", "toggles.json"), lookahead: file("radio_lookahead", "radio_lookahead.json") }
}

fn write_json(file: &Path, value: &Value) {
    let _ = atomic_write(file, &serde_json::to_string_pretty(value).unwrap_or_default());
}

// ------------------------------------------------------------------ settings ---

pub async fn get_env(sh: &Sh) -> Value {
    let result = run_core(sh, &["--env-json"], None, Duration::from_secs(30)).await;
    let values = last_json_line(&result.stdout).unwrap_or(Value::Null);
    let mut safe = Map::new();
    for key in ENV_KEYS {
        safe.insert(key.to_string(), values.get(key).cloned().unwrap_or_else(|| json!("")));
    }
    // Never hand the client secret to the page; it only needs to know it exists.
    let secret_set = safe.remove("TWITCH_CLIENT_SECRET").and_then(|v| v.as_str().map(|s| !s.is_empty())).unwrap_or(false);
    json!({ "values": safe, "secretSet": secret_set, "envPath": sh.paths.home.join(".env").to_string_lossy() })
}

/// Which of the values the page sent may be written: known keys only, trimmed, and a blank secret never erases the stored one.
pub fn env_updates(values: &Value) -> Map<String, Value> {
    let mut updates = Map::new();
    for (key, raw) in values.as_object().into_iter().flatten() {
        if !ENV_KEYS.contains(&key.as_str()) {
            continue;
        }
        let text = match raw {
            Value::String(s) => s.trim().to_string(),
            Value::Null => String::new(),
            other => other.to_string().trim_matches('"').trim().to_string(),
        };
        if key == "TWITCH_CLIENT_SECRET" && text.is_empty() {
            continue;
        }
        updates.insert(key.clone(), Value::String(text));
    }
    updates
}

pub async fn save_env(sh: &Sh, payload: &Value) -> Value {
    let _one_at_a_time = sh.save_lock.lock().await;
    let updates = env_updates(payload.get("values").unwrap_or(&Value::Null));
    if updates.is_empty() {
        let report = match last_preflight(sh) {
            Some(report) => report,
            None => preflight(sh, true).await,
        };
        return json!({ "ok": true, "preflight": report, "needsRestart": false });
    }
    let result = run_core(sh, &["--env-update-stdin"], Some(Value::Object(updates.clone()).to_string()), Duration::from_secs(30)).await;
    if result.code != 0 {
        let error = result.stderr.trim();
        return json!({ "ok": false, "error": if error.is_empty() { "Couldn't save the settings." } else { error } });
    }
    sh.logs.gui("INFO", &format!("Settings saved ({} value(s)).", updates.len()));
    let report = preflight(sh, true).await;
    let phase = sh.core.phase();
    json!({ "ok": true, "preflight": report, "needsRestart": !matches!(phase, core_proc::Phase::Stopped | core_proc::Phase::Error) })
}

// --------------------------------------------------------------- live limits ---

pub async fn get_live(sh: &Sh) -> Value {
    let files = live_files(sh).await;
    let saved = read_json_object(&files.tunables);
    let toggles = read_json_object(&files.toggles);
    let (bounds, defaults) = tunable_spec(sh);
    let mut tunables = Map::new();
    for key in bounds.keys() {
        let value = saved.get(key).and_then(parse_int_value).map(Value::from).or_else(|| defaults.get(key).cloned()).unwrap_or(Value::Null);
        tunables.insert(key.clone(), value);
    }
    json!({
        "tunables": tunables,
        "bounds": bounds,
        "radio_autoplay_enabled": toggles.get("radio_autoplay_enabled").and_then(Value::as_bool) != Some(false),
    })
}

pub async fn save_live(sh: &Sh, payload: &Value) -> Value {
    let files = live_files(sh).await;
    let mut current = read_json_object(&files.tunables);
    let (bounds, _) = tunable_spec(sh);
    for (key, range) in &bounds {
        let (Some(lo), Some(hi)) = (range.get(0).and_then(Value::as_i64), range.get(1).and_then(Value::as_i64)) else { continue };
        if let Some(n) = payload.pointer(&format!("/tunables/{key}")).and_then(parse_int_value) {
            current.insert(key.clone(), json!(n.clamp(lo, hi)));
        }
    }
    write_json(&files.tunables, &Value::Object(current));
    if let Some(on) = payload.get("radio_autoplay_enabled").and_then(Value::as_bool) {
        let mut toggles = read_json_object(&files.toggles);
        let changed = toggles.get("radio_autoplay_enabled").and_then(Value::as_bool) != Some(on);
        toggles.insert("radio_autoplay_enabled".into(), json!(on));
        write_json(&files.toggles, &Value::Object(toggles));
        // Auto-radio decides whether radio-mix songs may sit in the queue: tell a running bot now.
        if changed && sh.core.is_running_process() {
            core_proc::send_command(sh, "radio_lookahead", json!({}), Duration::from_secs(4)).await;
        }
    }
    sh.logs.gui("INFO", "Live limits saved.");
    get_live(sh).await
}

// ------------------------------------------------------------ radio lookahead ---
// Keep the next N songs of the radio mix queued (1 to 15). Changed from the dashboard only: the file is
// written here and read by the bot, never by chat commands or the browser settings page.

pub async fn get_lookahead(sh: &Sh) -> Value {
    let files = live_files(sh).await;
    normalize_lookahead(&Value::Object(read_json_object(&files.lookahead)))
}

pub async fn set_lookahead(sh: &Sh, value: &Value) -> Value {
    let files = live_files(sh).await;
    let mut merged = get_lookahead(sh).await;
    if let Some(patch) = value.as_object() {
        for (key, val) in patch {
            merged[key] = val.clone();
        }
    }
    let next = normalize_lookahead(&merged);
    write_json(&files.lookahead, &next);
    if sh.core.is_running_process() {
        core_proc::send_command(sh, "radio_lookahead", json!({}), Duration::from_secs(4)).await; // trims or tops up the queue right away
    }
    let message = if next["enabled"] == json!(true) { format!("Radio lookahead on: {} song(s).", next["count"]) } else { "Radio lookahead off.".to_string() };
    sh.logs.gui("INFO", &message);
    next
}

// -------------------------------------------------------------------- updates ---

pub fn app_version(sh: &Sh) -> String {
    sh.app.package_info().version.to_string()
}

pub async fn check_app_update_now(sh: &Sh) -> Value {
    let current = app_version(sh);
    let platform = crate::paths::platform();
    let result = tauri::async_runtime::spawn_blocking({
        let current = current.clone();
        move || check_for_app_update(&current, platform, UPDATE_REPO, &github_fetch)
    })
    .await
    .unwrap_or_else(|e| updates::UpdateResult {
        ok: false,
        current: current.clone(),
        latest: None,
        update_available: false,
        release_url: None,
        installer_url: None,
        error: Some(e.to_string()),
    });
    if !result.ok {
        sh.logs.gui("WARNING", &format!("App update check failed: {}", result.error.clone().unwrap_or_default()));
    } else if result.update_available {
        sh.logs.gui("INFO", &format!("PyxeeRadio {} is available (you have {current}). See Settings > App.", result.latest.clone().unwrap_or_default()));
    } else {
        sh.logs.gui("INFO", &format!("PyxeeRadio is up to date ({current})."));
    }
    *sh.app_update.lock().unwrap() = Some(result.clone());
    let value = serde_json::to_value(&result).unwrap_or(Value::Null);
    let _ = sh.app.emit("update:app", &value);
    value
}

/// Runs a yt-dlp update mode of the core; every attempt lands in the Logs tab.
async fn run_ytdlp_mode(sh: &Sh, flag: &str, timeout: Duration) -> Value {
    let result = run_core(sh, &[flag], None, timeout).await;
    let Some(mut report) = last_json_line(&result.stdout) else {
        let detail = tail(&if result.stderr.is_empty() { result.error.clone().unwrap_or_default() } else { result.stderr.clone() }, 300);
        sh.logs.gui("ERROR", &format!("yt-dlp {flag} gave no answer. {detail}"));
        return json!({ "ok": false, "error": "The bot core didn't answer. See the Logs tab." });
    };
    if let Some(lines) = report.as_object_mut().and_then(|o| o.remove("log")).and_then(|l| l.as_array().cloned()) {
        for line in lines {
            sh.logs.push(line.get("level").and_then(Value::as_str).unwrap_or("INFO"), "yt-dlp", line.get("msg").and_then(Value::as_str).unwrap_or(""), None, false);
        }
    }
    report
}

pub async fn ytdlp_check(sh: &Sh) -> Value {
    if sh.ytdlp_busy.swap(true, Ordering::SeqCst) {
        return json!({ "ok": false, "error": "Another yt-dlp operation is running." });
    }
    let report = run_ytdlp_mode(sh, "--ytdlp-check", Duration::from_secs(60)).await;
    sh.ytdlp_busy.store(false, Ordering::SeqCst);
    let _ = sh.app.emit("update:ytdlp", &report);
    report
}

/// Install or roll back: the bot is stopped around the swap and restarted if it was running.
pub async fn ytdlp_change(sh: &Sh, flag: &str) -> Value {
    if sh.ytdlp_busy.swap(true, Ordering::SeqCst) {
        return json!({ "ok": false, "error": "Another yt-dlp operation is running." });
    }
    let was_running = sh.core.is_running_process();
    if was_running {
        sh.logs.gui("INFO", "Stopping the bot to change the yt-dlp version.");
        core_proc::stop_bot(sh).await;
    }
    let report = run_ytdlp_mode(sh, flag, Duration::from_secs(180)).await;
    if was_running {
        core_proc::start_bot(sh).await;
    }
    sh.ytdlp_busy.store(false, Ordering::SeqCst);
    report
}

pub fn schedule_startup_checks(sh: &Sh) {
    let sh = sh.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_secs(8)).await;
        if updates::should_auto_check(&sh.prefs_snapshot(), now_ms()) {
            sh.update_prefs(|p| p.last_update_check = now_ms());
            check_app_update_now(&sh).await;
        }
        // Like the app check, at most once a day: every check starts a core process and asks GitHub.
        if updates::should_auto_check_ytdlp(&sh.prefs_snapshot(), now_ms()) {
            sh.update_prefs(|p| p.last_ytdlp_check = now_ms());
            let report = ytdlp_check(&sh).await;
            if report.get("ok") == Some(&json!(true)) && report.get("update_available") == Some(&json!(true)) {
                sh.logs.gui("INFO", &format!("A newer yt-dlp ({}) is available. Settings > App can install it.", report.get("latest").and_then(Value::as_str).unwrap_or("?")));
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_last_json_line_wins_and_warnings_before_it_are_ignored() {
        assert_eq!(last_json_line("warning: x\n{\"ok\": true}\n"), Some(json!({ "ok": true })));
        assert_eq!(last_json_line("{\"a\":1}\n{\"b\":2}"), Some(json!({ "b": 2 })));
        assert_eq!(last_json_line("no json here"), None);
        assert_eq!(last_json_line(""), None);
    }

    #[test]
    fn only_known_keys_are_written_trimmed_and_a_blank_secret_is_never_sent() {
        let updates = env_updates(&json!({
            "TWITCH_CLIENT_ID": "  abc ",
            "TWITCH_CLIENT_SECRET": "   ",
            "LOG_LEVEL": "DEBUG",
            "PATH": "/evil",
            "TWITCH_RADIO_HOME": "/evil",
            "YTDLP_CONCURRENCY": 3,
            "LOG_TO_FILE": null,
        }));
        assert_eq!(updates.get("TWITCH_CLIENT_ID"), Some(&json!("abc")));
        assert_eq!(updates.get("LOG_LEVEL"), Some(&json!("DEBUG")));
        assert_eq!(updates.get("YTDLP_CONCURRENCY"), Some(&json!("3")));
        assert_eq!(updates.get("LOG_TO_FILE"), Some(&json!("")));
        for refused in ["TWITCH_CLIENT_SECRET", "PATH", "TWITCH_RADIO_HOME"] {
            assert!(!updates.contains_key(refused), "{refused}");
        }
        let secret = env_updates(&json!({ "TWITCH_CLIENT_SECRET": " s3cret " }));
        assert_eq!(secret.get("TWITCH_CLIENT_SECRET"), Some(&json!("s3cret")));
    }

    #[test]
    fn tail_keeps_the_end_of_long_text() {
        assert_eq!(tail("  abcdef  ", 3), "def");
        assert_eq!(tail("ab", 10), "ab");
    }
}
