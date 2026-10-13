//! The bot: a hidden child process that speaks JSON lines on stdin/stdout (the protocol is documented
//! in twitch_radio/headless.py). This module starts and stops it, restarts it after a crash, hands it
//! commands and turns what it prints into events for the page and the Logs tab.

use crate::state::Sh;
use crate::updates::now_ms;
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::Ordering;
use std::sync::Mutex;
use std::time::Duration;
use tauri::{Emitter, Manager};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{ChildStdin, Command};
use tokio::sync::oneshot;

const STOP_GRACE: Duration = Duration::from_secs(20);

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Phase {
    Stopped,
    Starting,
    Running,
    Stopping,
    Error,
}

impl Phase {
    pub fn label(self) -> &'static str {
        match self {
            Phase::Stopped => "Stopped",
            Phase::Starting => "Starting...",
            Phase::Running => "Running",
            Phase::Stopping => "Stopping...",
            Phase::Error => "Error",
        }
    }
}

#[derive(Clone, Serialize, Debug)]
pub struct Fatal {
    pub message: String,
    pub hint: String,
    pub code: i64,
}

struct Inner {
    phase: Phase,
    fatal: Option<Fatal>,
    exit_code: Option<i64>,
    user_stopped: bool,
    pid: Option<u32>,
    running: bool,
    stop_waiters: Vec<oneshot::Sender<()>>,
    pending: HashMap<u64, oneshot::Sender<Value>>,
    ack_id: u64,
    last_state: Option<Value>,
    /// Whether the core has been told the dashboard is on screen.
    watching: bool,
    crash_times: Vec<u64>,
    restart_at: Option<u64>,
    started_at: Option<u64>,
    restart_task: Option<tauri::async_runtime::JoinHandle<()>>,
    stop_task: Option<tauri::async_runtime::JoinHandle<()>>,
    kill: Option<oneshot::Sender<()>>,
}

pub struct Core {
    inner: Mutex<Inner>,
    stdin: tokio::sync::Mutex<Option<ChildStdin>>,
}

impl Default for Core {
    fn default() -> Self {
        Self {
            inner: Mutex::new(Inner {
                phase: Phase::Stopped,
                fatal: None,
                exit_code: None,
                user_stopped: false,
                pid: None,
                running: false,
                stop_waiters: vec![],
                pending: HashMap::new(),
                ack_id: 0,
                last_state: None,
                watching: false,
                crash_times: vec![],
                restart_at: None,
                started_at: None,
                restart_task: None,
                stop_task: None,
                kill: None,
            }),
            stdin: tokio::sync::Mutex::new(None),
        }
    }
}

impl Core {
    pub fn phase(&self) -> Phase {
        self.inner.lock().unwrap().phase
    }
    pub fn is_running_process(&self) -> bool {
        self.inner.lock().unwrap().running
    }
    pub fn last_state(&self) -> Option<Value> {
        self.inner.lock().unwrap().last_state.clone()
    }
    pub fn pid(&self) -> Option<u32> {
        self.inner.lock().unwrap().pid
    }
}

pub fn status(sh: &Sh) -> Value {
    let inner = sh.core.inner.lock().unwrap();
    json!({
        "phase": inner.phase,
        "pid": inner.pid,
        "fatal": inner.fatal,
        "exitCode": inner.exit_code,
        "restartAt": inner.restart_at,
        "startedAt": inner.started_at,
        "home": sh.paths.home.to_string_lossy(),
    })
}

fn set_phase(sh: &Sh, phase: Phase) {
    {
        let mut inner = sh.core.inner.lock().unwrap();
        if inner.phase == phase {
            return;
        }
        inner.phase = phase;
    }
    broadcast_status(sh);
}

pub fn broadcast_status(sh: &Sh) {
    let _ = sh.app.emit("bot:status", status(sh));
    crate::tray::refresh(sh);
}

fn fail_start(sh: &Sh, message: String) -> Value {
    sh.core.inner.lock().unwrap().fatal = Some(Fatal {
        message: message.clone(),
        hint: "Reinstall the app, or check that antivirus did not quarantine PyxeeRadioCore.exe.".into(),
        code: 3,
    });
    sh.logs.gui("ERROR", &message);
    set_phase(sh, Phase::Error);
    status(sh)
}

/// Starts the bot. Boxed because a crash restarts it from inside its own exit handler (an async
/// function that, through a spawned task, awaits itself has no nameable type).
pub fn start_bot(sh: &Sh) -> Pin<Box<dyn Future<Output = Value> + Send>> {
    let sh = sh.clone();
    Box::pin(async move { start_bot_now(&sh).await })
}

async fn start_bot_now(sh: &Sh) -> Value {
    {
        let mut inner = sh.core.inner.lock().unwrap();
        if inner.running {
            drop(inner);
            return status(sh);
        }
        if let Some(task) = inner.restart_task.take() {
            task.abort();
        }
        inner.restart_at = None;
        inner.fatal = None;
        inner.exit_code = None;
        inner.user_stopped = false;
        inner.last_state = None;
        inner.watching = false;
        inner.started_at = Some(now_ms());
    }
    let (program, mut args) = sh.paths.core_command();
    args.push("--headless".into());
    let file_name = program.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    sh.logs.gui("INFO", &format!("Starting the bot ({file_name})."));

    let mut command = Command::new(&program);
    command
        .args(&args)
        .envs(sh.paths.core_env())
        .current_dir(sh.paths.core_cwd())
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    hide_console(&mut command);
    #[cfg(unix)]
    command.process_group(0); // its own group, so the whole tree can be stopped together
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => return fail_start(sh, format!("Couldn't launch the bot: {error}")),
    };
    let (kill_tx, kill_rx) = oneshot::channel();
    {
        let mut inner = sh.core.inner.lock().unwrap();
        inner.running = true;
        inner.pid = child.id();
        inner.kill = Some(kill_tx);
    }
    *sh.core.stdin.lock().await = child.stdin.take();
    set_phase(sh, Phase::Starting);

    if let Some(stdout) = child.stdout.take() {
        let sh = sh.clone();
        tauri::async_runtime::spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                on_core_line(&sh, &line).await;
            }
        });
    }
    if let Some(stderr) = child.stderr.take() {
        let sh = sh.clone();
        tauri::async_runtime::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                if line.trim().is_empty() {
                    continue;
                }
                let level = if ["Traceback", "Error", "Exception"].iter().any(|word| line.contains(word)) { "ERROR" } else { "WARNING" };
                sh.logs.push(level, "stderr", &line, None, false);
            }
        });
    }
    {
        let sh = sh.clone();
        let pid = child.id();
        tauri::async_runtime::spawn(async move {
            let status = tokio::select! {
                status = child.wait() => status,
                _ = kill_rx => {
                    if let Some(pid) = pid { kill_tree(pid).await; }
                    child.wait().await
                }
            };
            on_core_exit(&sh, status).await;
        });
    }
    update_watching(sh, true);
    status(sh)
}

pub(crate) fn hide_console(command: &mut Command) {
    #[cfg(windows)]
    command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    #[cfg(not(windows))]
    let _ = command;
}

async fn kill_tree(pid: u32) {
    #[cfg(windows)]
    {
        let mut command = Command::new("taskkill");
        command.args(["/pid", &pid.to_string(), "/T", "/F"]);
        hide_console(&mut command);
        let _ = command.status().await;
    }
    #[cfg(unix)]
    {
        // The core leads its own process group (see start_bot): this takes ffmpeg and the lookup workers with it.
        let _ = Command::new("kill").args(["-KILL", "--", &format!("-{pid}")]).status().await;
    }
}

/// Synchronous last resort for when the app is already going away.
pub fn kill_now(sh: &Sh) {
    if let Some(tx) = sh.core.inner.lock().unwrap().kill.take() {
        let _ = tx.send(());
    }
    if let Some(pid) = sh.core.pid() {
        #[cfg(windows)]
        let _ = std::process::Command::new("taskkill").args(["/pid", &pid.to_string(), "/T", "/F"]).status();
        #[cfg(unix)]
        let _ = std::process::Command::new("kill").args(["-KILL", "--", &format!("-{pid}")]).status();
    }
}

async fn on_core_line(sh: &Sh, line: &str) {
    if line.trim().is_empty() {
        return;
    }
    let Ok(msg) = serde_json::from_str::<Value>(line) else {
        sh.logs.push("INFO", "core", line, None, false);
        return;
    };
    match msg.get("t").and_then(Value::as_str) {
        Some("log") => sh.logs.push_core(&msg),
        Some("phase") => match msg.get("phase").and_then(Value::as_str) {
            Some("running") => set_phase(sh, Phase::Running),
            Some("stopping") => set_phase(sh, Phase::Stopping),
            Some("starting") if sh.core.phase() == Phase::Stopped => set_phase(sh, Phase::Starting),
            _ => {}
        },
        Some("state") => {
            sh.core.inner.lock().unwrap().last_state = Some(msg.clone());
            let _ = sh.app.emit("bot:state", &msg);
        }
        Some("fatal") => {
            let text = |key: &str| msg.get(key).and_then(Value::as_str).unwrap_or("").to_string();
            sh.core.inner.lock().unwrap().fatal =
                Some(Fatal { message: text("message"), hint: text("hint"), code: msg.get("code").and_then(Value::as_i64).unwrap_or(3) });
        }
        Some("ack") => {
            let waiter = msg.get("id").and_then(Value::as_u64).and_then(|id| sh.core.inner.lock().unwrap().pending.remove(&id));
            if let Some(waiter) = waiter {
                let _ = waiter.send(msg);
            }
        }
        _ => {} // hello and anything newer
    }
}

async fn on_core_exit(sh: &Sh, result: std::io::Result<std::process::ExitStatus>) {
    let code = result.as_ref().ok().and_then(|s| s.code()).map(i64::from);
    #[cfg(unix)]
    let signal = {
        use std::os::unix::process::ExitStatusExt;
        result.as_ref().ok().and_then(|s| s.signal())
    };
    #[cfg(not(unix))]
    let signal: Option<i32> = None;
    *sh.core.stdin.lock().await = None;

    let (clean, waiters, acks, fatal_code, user_stopped) = {
        let mut inner = sh.core.inner.lock().unwrap();
        if let Some(task) = inner.stop_task.take() {
            task.abort();
        }
        inner.running = false;
        inner.pid = None;
        inner.kill = None;
        inner.exit_code = code;
        inner.last_state = None;
        let clean = inner.user_stopped || code == Some(0);
        if !clean && inner.fatal.is_none() {
            inner.fatal = Some(Fatal {
                message: format!("The bot stopped unexpectedly (exit code {}).", code.map_or("unknown".to_string(), |c| c.to_string())),
                hint: "See the Logs tab for details.".into(),
                code: code.unwrap_or(3),
            });
        }
        (clean, std::mem::take(&mut inner.stop_waiters), std::mem::take(&mut inner.pending), inner.fatal.as_ref().map(|f| f.code), inner.user_stopped)
    };
    for (_, waiter) in acks {
        let _ = waiter.send(json!({ "ok": false, "error": "bot stopped" }));
    }
    let how = match (signal, code) {
        (Some(signal), _) => format!("signal {signal}"),
        (None, Some(code)) => format!("code {code}"),
        _ => "no exit code".to_string(),
    };
    sh.logs.gui(if clean { "INFO" } else { "ERROR" }, &format!("The bot {} ({how}).", if clean { "stopped" } else { "exited unexpectedly" }));
    set_phase(sh, if clean { Phase::Stopped } else { Phase::Error });
    let _ = sh.app.emit("bot:state", Value::Null);
    for waiter in waiters {
        let _ = waiter.send(());
    }

    // Retry only what might fix itself (network down), never config errors, and never in a tight loop.
    let auto_restart = sh.prefs.lock().unwrap().auto_restart;
    if !clean && !user_stopped && auto_restart && fatal_code != Some(2) && !sh.quitting.load(Ordering::SeqCst) {
        let now = now_ms();
        let (attempts, delay_ms) = {
            let mut inner = sh.core.inner.lock().unwrap();
            inner.crash_times.retain(|t| now.saturating_sub(*t) < 10 * 60 * 1000);
            inner.crash_times.push(now);
            let attempts = inner.crash_times.len();
            (attempts, (3000u64 << (attempts - 1).min(10)).min(60_000))
        };
        if attempts <= 5 {
            sh.logs.gui("WARNING", &format!("Restarting automatically in {}s (attempt {attempts} of 5).", delay_ms / 1000));
            let task = {
                let sh = sh.clone();
                tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                    start_bot(&sh).await;
                })
            };
            {
                let mut inner = sh.core.inner.lock().unwrap();
                inner.restart_at = Some(now + delay_ms);
                inner.restart_task = Some(task);
            }
            broadcast_status(sh);
        } else {
            sh.logs.gui("ERROR", "Giving up on automatic restarts after 5 failures in 10 minutes.");
        }
    }
}

/// Asks the bot to shut down cleanly and resolves once it has exited (or been forced to after 20 seconds).
pub async fn stop_bot(sh: &Sh) {
    let waiter = {
        let mut inner = sh.core.inner.lock().unwrap();
        if let Some(task) = inner.restart_task.take() {
            task.abort();
        }
        inner.restart_at = None;
        if !inner.running {
            let was_error = inner.phase == Phase::Error;
            drop(inner);
            if was_error {
                set_phase(sh, Phase::Stopped);
            } else {
                broadcast_status(sh); // a pending automatic restart was just cancelled
            }
            return;
        }
        let (tx, rx) = oneshot::channel();
        inner.stop_waiters.push(tx);
        let first = !inner.user_stopped;
        inner.user_stopped = true;
        (rx, first)
    };
    let (rx, first) = waiter;
    if first {
        set_phase(sh, Phase::Stopping);
        sh.logs.gui("INFO", "Stop requested - shutting down gracefully.");
        if write_line(sh, &json!({ "cmd": "stop" })).await.is_err() {
            force_kill(sh);
        }
        let task = {
            let sh = sh.clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(STOP_GRACE).await;
                sh.logs.gui("WARNING", &format!("The bot didn't stop within {}s - forcing it to close.", STOP_GRACE.as_secs()));
                force_kill(&sh);
            })
        };
        sh.core.inner.lock().unwrap().stop_task = Some(task);
    }
    let _ = rx.await;
}

fn force_kill(sh: &Sh) {
    if let Some(tx) = sh.core.inner.lock().unwrap().kill.take() {
        let _ = tx.send(());
    }
}

pub async fn restart_bot(sh: &Sh) -> Value {
    stop_bot(sh).await;
    start_bot(sh).await
}

async fn write_line(sh: &Sh, value: &Value) -> std::io::Result<()> {
    let mut guard = sh.core.stdin.lock().await;
    let stdin = guard.as_mut().ok_or_else(|| std::io::Error::new(std::io::ErrorKind::BrokenPipe, "no stdin"))?;
    stdin.write_all(format!("{value}\n").as_bytes()).await?;
    stdin.flush().await
}

/// Sends one command to the running bot and resolves with its answer. `extra` fields ride along (a song
/// query, say); a lookup can take a while, hence the timeout argument.
pub async fn send_command(sh: &Sh, name: &str, extra: Value, timeout: Duration) -> Value {
    if !sh.core.is_running_process() {
        return json!({ "ok": false, "error": "The bot is not running." });
    }
    let (id, rx) = {
        let mut inner = sh.core.inner.lock().unwrap();
        inner.ack_id += 1;
        let id = inner.ack_id;
        let (tx, rx) = oneshot::channel();
        inner.pending.insert(id, tx);
        (id, rx)
    };
    let mut message = if extra.is_object() { extra } else { json!({}) };
    message["cmd"] = json!(name);
    message["id"] = json!(id);
    if write_line(sh, &message).await.is_err() {
        sh.core.inner.lock().unwrap().pending.remove(&id);
        return json!({ "ok": false, "error": "The bot is not accepting commands." });
    }
    match tokio::time::timeout(timeout, rx).await {
        Ok(Ok(ack)) => ack,
        Ok(Err(_)) => json!({ "ok": false, "error": "bot stopped" }),
        Err(_) => {
            sh.core.inner.lock().unwrap().pending.remove(&id);
            json!({ "ok": false, "error": "No answer from the bot." })
        }
    }
}

/// Tells the core whether the dashboard is on screen. Hidden, the core sends a state message only when
/// the player's state actually changes; visible, it also sends a slow heartbeat. `force` re-sends the
/// current answer (a fresh core starts out assuming nobody is watching).
pub fn update_watching(sh: &Sh, force: bool) {
    let visible = sh
        .app
        .get_webview_window("main")
        .is_some_and(|w| w.is_visible().unwrap_or(false) && !w.is_minimized().unwrap_or(false));
    {
        let mut inner = sh.core.inner.lock().unwrap();
        if !force && visible == inner.watching {
            return;
        }
        inner.watching = visible;
        if !inner.running {
            return;
        }
    }
    let sh = sh.clone();
    tauri::async_runtime::spawn(async move {
        let _ = write_line(&sh, &json!({ "cmd": "watch", "on": visible })).await;
    });
}

/// For when the app is going away without the usual Quit (a session ending, say): closing the control
/// channel makes the core run its graceful stop on its own (it saves the queue and tokens); if it has
/// not finished in a few seconds it is killed, so nothing is left behind.
pub fn shutdown_blocking(sh: &Sh) {
    if !sh.core.is_running_process() {
        return;
    }
    tauri::async_runtime::block_on(async {
        *sh.core.stdin.lock().await = None;
        for _ in 0..80 {
            if !sh.core.is_running_process() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    });
    kill_now(sh);
}
