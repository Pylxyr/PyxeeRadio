//! Where things live: the data folder, the bot core, the bundled tools.

use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};

pub const IS_WIN: bool = cfg!(windows);

#[derive(Clone, Debug)]
pub struct Paths {
    /// The bot's data folder (.env, data/, logs/).
    pub home: PathBuf,
    /// The source checkout; only meaningful when running from source (not packaged).
    pub project_root: PathBuf,
    /// True when the bot core (a PyInstaller build) is installed next to the app.
    pub packaged: bool,
    /// Where core/ and bin/ were found when packaged.
    pub resources: Option<PathBuf>,
    /// The window's own files (gui-settings.json), beside the bot's data rather than in it.
    pub gui_dir: PathBuf,
}

fn core_exe_name() -> &'static str {
    if IS_WIN {
        "PyxeeRadioCore.exe"
    } else {
        "PyxeeRadioCore"
    }
}

/// The folder that holds `core/`: next to the executable (Windows installer, the Linux archive), under
/// ../lib/<name> (a .deb), or wherever Tauri says the resources are.
fn find_resources(app: &AppHandle) -> Option<PathBuf> {
    let exe_dir = std::env::current_exe().ok()?.parent()?.to_path_buf();
    let mut candidates = vec![exe_dir.clone(), exe_dir.join("resources"), exe_dir.join("../lib/pyxee-radio"), exe_dir.join("../lib/PyxeeRadio")];
    if let Ok(dir) = app.path().resource_dir() {
        candidates.push(dir);
    }
    candidates.into_iter().find(|dir| dir.join("core").join(core_exe_name()).is_file())
}

impl Paths {
    pub fn detect(app: &AppHandle) -> Self {
        let project_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
        let project_root = project_root.canonicalize().unwrap_or(project_root);
        let resources = find_resources(app);
        let packaged = resources.is_some();
        let config_dir = app.path().config_dir().unwrap_or_else(|_| std::env::temp_dir());
        let home = match std::env::var("TWITCH_RADIO_HOME") {
            Ok(value) if !value.trim().is_empty() => PathBuf::from(value),
            _ if packaged => config_dir.join("TwitchRadio"),
            _ => project_root.clone(),
        };
        Self { home, project_root, packaged, resources, gui_dir: config_dir.join("TwitchRadio").join("gui") }
    }

    pub fn prefs_file(&self) -> PathBuf {
        self.gui_dir.join("gui-settings.json")
    }

    /// The program to run for the bot, and the arguments that go before the mode flag.
    pub fn core_command(&self) -> (PathBuf, Vec<String>) {
        if let Some(resources) = &self.resources {
            return (resources.join("core").join(core_exe_name()), vec![]);
        }
        let python = std::env::var("TWITCH_RADIO_PYTHON").ok().filter(|p| !p.trim().is_empty()).map(PathBuf::from).unwrap_or_else(|| {
            let venv = self.project_root.join(".venv").join(if IS_WIN { "Scripts/python.exe" } else { "bin/python" });
            if venv.exists() {
                venv
            } else {
                PathBuf::from(if IS_WIN { "python" } else { "python3" })
            }
        });
        (python, vec![self.project_root.join("bot.py").to_string_lossy().into_owned()])
    }

    /// ffmpeg and the other bundled tools.
    pub fn bin_dir(&self) -> Option<PathBuf> {
        let dir = match &self.resources {
            Some(resources) => resources.join("bin"),
            None => self.project_root.join("packaging").join("bin"),
        };
        dir.exists().then_some(dir)
    }

    pub fn core_env(&self) -> Vec<(String, String)> {
        let mut env = vec![
            ("TWITCH_RADIO_HOME".to_string(), self.home.to_string_lossy().into_owned()),
            ("PYTHONIOENCODING".to_string(), "utf-8".to_string()),
            ("PYTHONUNBUFFERED".to_string(), "1".to_string()),
        ];
        if let Some(bin) = self.bin_dir() {
            env.push(("TWITCH_RADIO_BIN".to_string(), bin.to_string_lossy().into_owned()));
        }
        env
    }

    /// A working directory for the core that exists: the data folder, or the checkout if that cannot be made.
    pub fn core_cwd(&self) -> PathBuf {
        if std::fs::create_dir_all(&self.home).is_ok() {
            self.home.clone()
        } else {
            self.project_root.clone()
        }
    }
}

/// "windows" | "linux" | "macos": what the page uses to decide on window controls and per-OS settings.
pub fn platform() -> &'static str {
    match std::env::consts::OS {
        "windows" => "windows",
        "macos" => "macos",
        _ => "linux",
    }
}
