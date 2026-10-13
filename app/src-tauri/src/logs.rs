//! The Logs tab's buffer: what the bot prints, what its process writes to stderr, and the shell's own
//! notes. The newest LOG_LIMIT lines are kept; the page gets them in batches so a burst of lines is
//! one message, not hundreds.

use serde::Serialize;
use serde_json::Value;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter};

const LOG_LIMIT: usize = 5000;
const LOG_FLUSH: Duration = Duration::from_millis(100);

#[derive(Serialize, Clone, Debug)]
pub struct LogEntry {
    pub id: u64,
    pub ts: f64,
    pub level: String,
    pub logger: String,
    pub msg: String,
    pub cont: bool,
}

#[derive(Default)]
struct Inner {
    seq: u64,
    buffer: VecDeque<LogEntry>,
    pending: Vec<LogEntry>,
    flush_scheduled: bool,
}

#[derive(Clone)]
pub struct Logs {
    app: AppHandle,
    inner: Arc<Mutex<Inner>>,
}

fn now_secs() -> f64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0.0, |d| d.as_secs_f64())
}

impl Logs {
    pub fn new(app: AppHandle) -> Self {
        Self { app, inner: Arc::default() }
    }

    pub fn push(&self, level: &str, logger: &str, msg: &str, ts: Option<f64>, cont: bool) {
        let schedule = {
            let mut inner = self.inner.lock().unwrap();
            inner.seq += 1;
            let entry = LogEntry {
                id: inner.seq,
                ts: ts.unwrap_or_else(now_secs),
                level: level.to_uppercase(),
                logger: logger.to_string(),
                msg: msg.to_string(),
                cont,
            };
            inner.buffer.push_back(entry.clone());
            while inner.buffer.len() > LOG_LIMIT {
                inner.buffer.pop_front();
            }
            inner.pending.push(entry);
            !std::mem::replace(&mut inner.flush_scheduled, true)
        };
        if schedule {
            let (app, inner) = (self.app.clone(), self.inner.clone());
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(LOG_FLUSH).await;
                let batch = {
                    let mut inner = inner.lock().unwrap();
                    inner.flush_scheduled = false;
                    std::mem::take(&mut inner.pending)
                };
                if !batch.is_empty() {
                    let _ = app.emit("logs:batch", batch);
                }
            });
        }
    }

    /// A line the core printed as a JSON `{"t": "log", ...}` message.
    pub fn push_core(&self, msg: &Value) {
        let text = |key: &str| msg.get(key).and_then(Value::as_str);
        self.push(
            text("level").unwrap_or("INFO"),
            text("logger").unwrap_or("gui"),
            text("msg").unwrap_or(""),
            msg.get("ts").and_then(Value::as_f64),
            msg.get("cont").and_then(Value::as_bool).unwrap_or(false),
        );
    }

    /// The shell's own notes ("gui" in the Logs tab).
    pub fn gui(&self, level: &str, msg: &str) {
        self.push(level, "gui", msg, None, false);
    }

    pub fn all(&self) -> Vec<LogEntry> {
        self.inner.lock().unwrap().buffer.iter().cloned().collect()
    }

    pub fn clear(&self) {
        let mut inner = self.inner.lock().unwrap();
        inner.buffer.clear();
        inner.pending.clear();
    }
}

/// One exported line: `2026-10-09T12:00:00.000Z [INFO] core: text`.
pub fn export_line(entry: &LogEntry) -> String {
    let secs = entry.ts.floor() as i64;
    let millis = ((entry.ts - entry.ts.floor()) * 1000.0) as i64;
    let (y, mo, d, h, mi, s) = civil_from_unix(secs);
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{s:02}.{millis:03}Z [{}] {}: {}", entry.level, entry.logger, entry.msg)
}

/// Unix seconds to (year, month, day, hour, minute, second) in UTC (the civil-calendar algorithm, no time crate needed).
fn civil_from_unix(secs: i64) -> (i64, u32, u32, u32, u32, u32) {
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y, m, d, (rem / 3600) as u32, (rem % 3600 / 60) as u32, (rem % 60) as u32)
}

pub fn file_stamp(ts: f64) -> String {
    let (y, mo, d, h, mi, s) = civil_from_unix(ts as i64);
    format!("{y:04}-{mo:02}-{d:02}T{h:02}-{mi:02}-{s:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_come_out_right() {
        assert_eq!(civil_from_unix(0), (1970, 1, 1, 0, 0, 0));
        assert_eq!(civil_from_unix(951_782_400), (2000, 2, 29, 0, 0, 0)); // a leap day
        assert_eq!(civil_from_unix(1_760_000_000), (2025, 10, 9, 8, 53, 20));
        assert_eq!(civil_from_unix(-1), (1969, 12, 31, 23, 59, 59));
    }

    #[test]
    fn an_exported_line_has_time_level_logger_and_text() {
        let entry = LogEntry { id: 1, ts: 1_760_000_000.25, level: "INFO".into(), logger: "core".into(), msg: "hello".into(), cont: false };
        assert_eq!(export_line(&entry), "2025-10-09T08:53:20.250Z [INFO] core: hello");
        assert_eq!(file_stamp(1_760_000_000.0), "2025-10-09T08-53-20");
    }
}
