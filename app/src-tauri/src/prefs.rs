//! The window's own preferences (gui-settings.json). The page may change the ones listed in
//! `PAGE_KEYS` and nothing else; the update-check times and the remembered window colour belong to
//! the shell.

use crate::validate::{is_hex_color, is_skin, is_theme};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::Path;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Prefs {
    /// When the bot is running and the window is closed.
    pub ask_on_close: bool,
    /// Remembered answer when ask_on_close is off: "tray" | "quit".
    pub close_action: String,
    /// Restart after a crash (never after a config error).
    pub auto_restart: bool,
    pub start_bot_on_launch: bool,
    /// "system" | "light" | "dark".
    pub theme: String,
    /// Colour style, see app/shared/skin-names.json.
    pub skin: String,
    pub launch_at_login: bool,
    /// At most once a day.
    pub check_app_updates: bool,
    pub check_ytdlp_on_startup: bool,
    /// Set by the shell itself (milliseconds since the epoch), not by the page.
    pub last_update_check: u64,
    pub last_ytdlp_check: u64,
    /// What the window looked like last time (set by the page): the window is given this colour
    /// before the page has painted, so opening it never flashes the wrong one.
    pub window_bg: String,
    pub window_dark: bool,
}

impl Default for Prefs {
    fn default() -> Self {
        Self {
            ask_on_close: true,
            close_action: "tray".into(),
            auto_restart: true,
            start_bot_on_launch: false,
            theme: "system".into(),
            skin: "default".into(),
            launch_at_login: false,
            check_app_updates: true,
            check_ytdlp_on_startup: true,
            last_update_check: 0,
            last_ytdlp_check: 0,
            window_bg: "#0b0b10".into(),
            window_dark: true,
        }
    }
}

const BOOL_KEYS: [&str; 6] =
    ["askOnClose", "autoRestart", "startBotOnLaunch", "launchAtLogin", "checkAppUpdates", "checkYtdlpOnStartup"];

impl Prefs {
    /// Reads the file; anything missing, unreadable or invalid falls back to the defaults.
    pub fn load(file: &Path) -> Self {
        let mut prefs: Prefs = std::fs::read_to_string(file)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default();
        prefs.sanitize();
        prefs
    }

    pub fn save(&self, file: &Path) -> std::io::Result<()> {
        crate::fsutil::atomic_write(file, &serde_json::to_string_pretty(self).unwrap_or_default())
    }

    fn sanitize(&mut self) {
        let defaults = Prefs::default();
        if !is_theme(&self.theme) {
            self.theme = defaults.theme;
        }
        if !is_skin(&self.skin) {
            self.skin = defaults.skin;
        }
        if self.close_action != "tray" && self.close_action != "quit" {
            self.close_action = defaults.close_action;
        }
        if !is_hex_color(&self.window_bg) {
            self.window_bg = defaults.window_bg;
        }
    }

    /// Applies what the page asked for: only the keys it may change, and only values of the right kind.
    pub fn apply_patch(&mut self, patch: &Value) {
        let Some(map) = patch.as_object() else { return };
        for key in BOOL_KEYS {
            if let Some(value) = map.get(key).and_then(Value::as_bool) {
                match key {
                    "askOnClose" => self.ask_on_close = value,
                    "autoRestart" => self.auto_restart = value,
                    "startBotOnLaunch" => self.start_bot_on_launch = value,
                    "launchAtLogin" => self.launch_at_login = value,
                    "checkAppUpdates" => self.check_app_updates = value,
                    _ => self.check_ytdlp_on_startup = value,
                }
            }
        }
        if let Some(value) = map.get("theme").and_then(Value::as_str) {
            self.theme = value.to_string();
        }
        if let Some(value) = map.get("skin").and_then(Value::as_str) {
            self.skin = value.to_string();
        }
        if let Some(value) = map.get("closeAction").and_then(Value::as_str) {
            self.close_action = value.to_string();
        }
        self.sanitize();
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_missing_or_broken_file_gives_the_defaults() {
        let dir = tempdir("prefs-missing");
        assert_eq!(Prefs::load(&dir.join("nope.json")), Prefs::default());
        std::fs::write(dir.join("bad.json"), "{ not json").unwrap();
        assert_eq!(Prefs::load(&dir.join("bad.json")), Prefs::default());
    }

    #[test]
    fn an_old_file_keeps_its_choices_and_drops_what_no_longer_exists() {
        let dir = tempdir("prefs-old");
        let file = dir.join("gui-settings.json");
        std::fs::write(
            &file,
            r##"{"askOnClose": false, "closeAction": "quit", "theme": "dark", "skin": "ocean", "volume": 0.3, "mode": "player", "lastUpdateCheck": 5}"##,
        )
        .unwrap();
        let prefs = Prefs::load(&file);
        assert!(!prefs.ask_on_close);
        assert_eq!((prefs.close_action.as_str(), prefs.theme.as_str(), prefs.skin.as_str()), ("quit", "dark", "ocean"));
        assert_eq!(prefs.last_update_check, 5);
        assert!(prefs.auto_restart, "what the file does not say stays at its default");
    }

    #[test]
    fn invalid_values_fall_back() {
        let dir = tempdir("prefs-invalid");
        let file = dir.join("p.json");
        std::fs::write(&file, r##"{"theme": "neon", "skin": "__proto__", "closeAction": "explode", "windowBg": "red"}"##).unwrap();
        let prefs = Prefs::load(&file);
        assert_eq!((prefs.theme.as_str(), prefs.skin.as_str(), prefs.close_action.as_str()), ("system", "default", "tray"));
        assert_eq!(prefs.window_bg, Prefs::default().window_bg);
    }

    #[test]
    fn the_page_can_change_its_own_keys_with_the_right_kind_of_value_and_no_others() {
        let mut prefs = Prefs::default();
        prefs.apply_patch(&json!({
            "autoRestart": false,
            "theme": "light",
            "skin": "forest",
            "askOnClose": "no",            // wrong kind: ignored
            "lastUpdateCheck": 99,         // the shell's own: ignored
            "windowBg": "#ffffff",         // the shell's own: ignored
            "launchAtLogin": true,
        }));
        assert!(!prefs.auto_restart && prefs.launch_at_login);
        assert_eq!((prefs.theme.as_str(), prefs.skin.as_str()), ("light", "forest"));
        assert!(prefs.ask_on_close);
        assert_eq!(prefs.last_update_check, 0);
        assert_eq!(prefs.window_bg, Prefs::default().window_bg);
        prefs.apply_patch(&json!({ "theme": "neon", "skin": "nope" }));
        assert_eq!((prefs.theme.as_str(), prefs.skin.as_str()), ("system", "default"));
        prefs.apply_patch(&json!("not an object"));
    }

    #[test]
    fn saving_and_loading_round_trip() {
        let dir = tempdir("prefs-roundtrip");
        let file = dir.join("deep").join("gui-settings.json");
        let mut prefs = Prefs::default();
        prefs.apply_patch(&json!({ "theme": "dark", "skin": "rose" }));
        prefs.save(&file).unwrap();
        assert_eq!(Prefs::load(&file), prefs);
    }

    pub(crate) fn tempdir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("twitch-radio-test-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }
}
