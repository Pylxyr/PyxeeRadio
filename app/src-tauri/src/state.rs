//! What the shell shares between its commands, its tray and its background tasks.

use crate::{core_proc::Core, logs::Logs, paths::Paths, prefs::Prefs, tray::TrayItems, updates::UpdateResult};
use serde_json::Value;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tauri::AppHandle;

pub struct Shared {
    pub app: AppHandle,
    pub paths: Paths,
    pub prefs: Mutex<Prefs>,
    pub logs: Logs,
    pub core: Core,
    /// Set once the app is really going away (Quit, or the last window closed with nothing running).
    pub quitting: AtomicBool,
    pub app_update: Mutex<Option<UpdateResult>>,
    pub ytdlp_busy: AtomicBool,
    /// The last preflight report and when it was made.
    pub preflight: Mutex<Option<(Value, Instant)>>,
    /// Settings saves run one at a time: each is a read-modify-write of .env in a separate core process.
    pub save_lock: tokio::sync::Mutex<()>,
    pub tray: Mutex<Option<TrayItems>>,
}

pub type Sh = Arc<Shared>;

impl Shared {
    pub fn new(app: AppHandle) -> Sh {
        let paths = Paths::detect(&app);
        let prefs = Prefs::load(&paths.prefs_file());
        Arc::new(Self {
            logs: Logs::new(app.clone()),
            app,
            paths,
            prefs: Mutex::new(prefs),
            core: Core::default(),
            quitting: AtomicBool::new(false),
            app_update: Mutex::new(None),
            ytdlp_busy: AtomicBool::new(false),
            preflight: Mutex::new(None),
            save_lock: tokio::sync::Mutex::new(()),
            tray: Mutex::new(None),
        })
    }

    pub fn prefs_snapshot(&self) -> Prefs {
        self.prefs.lock().unwrap().clone()
    }

    /// Runs `change` on the prefs, saves them, and returns the result.
    pub fn update_prefs(&self, change: impl FnOnce(&mut Prefs)) -> Prefs {
        let mut prefs = self.prefs.lock().unwrap();
        change(&mut prefs);
        if let Err(error) = prefs.save(&self.paths.prefs_file()) {
            self.logs.gui("WARNING", &format!("Couldn't save the window settings: {error}"));
        }
        prefs.clone()
    }
}
