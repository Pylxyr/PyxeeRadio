//! App self-update check: asks the GitHub Releases API for the latest release and compares it with
//! the running version. It only reports and links; nothing is downloaded or installed here.

use crate::prefs::Prefs;
use serde::Serialize;
use serde_json::Value;
use std::cmp::Ordering;
use std::time::Duration;

pub const UPDATE_REPO: &str = "Pylxyr/Twitch-Radio";
pub const DAY_MS: u64 = 24 * 60 * 60 * 1000;

#[derive(Debug, PartialEq, Eq)]
pub struct Version {
    core: [u64; 3],
    pre: Option<Vec<String>>,
}

fn is_ident(text: &str) -> bool {
    !text.is_empty() && text.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-')
}

/// "1.2.3", "v1.2.3", "1.2.3-rc.1", "1.2.3+build": semantic versions, nothing else.
pub fn parse_version(text: &str) -> Option<Version> {
    let text = text.trim();
    let text = text.strip_prefix('v').unwrap_or(text);
    let (rest, build) = match text.split_once('+') {
        Some((rest, build)) => (rest, Some(build)),
        None => (text, None),
    };
    if build.is_some_and(|b| !is_ident(b)) {
        return None;
    }
    let (core_text, pre_text) = match rest.split_once('-') {
        Some((core, pre)) => (core, Some(pre)),
        None => (rest, None),
    };
    if pre_text.is_some_and(|p| !is_ident(p)) {
        return None;
    }
    let mut numbers = core_text.split('.');
    let mut core = [0u64; 3];
    for slot in &mut core {
        let part = numbers.next()?;
        if part.is_empty() || !part.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        *slot = part.parse().ok()?;
    }
    if numbers.next().is_some() {
        return None;
    }
    Some(Version { core, pre: pre_text.map(|p| p.split('.').map(str::to_string).collect()) })
}

fn is_numeric(part: &str) -> bool {
    !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit())
}

fn compare_prerelease(a: &Option<Vec<String>>, b: &Option<Vec<String>>) -> Ordering {
    match (a, b) {
        (None, None) => Ordering::Equal,
        (None, Some(_)) => Ordering::Greater, // a release outranks its own pre-releases
        (Some(_), None) => Ordering::Less,
        (Some(a), Some(b)) => {
            for i in 0..a.len().max(b.len()) {
                let (Some(x), Some(y)) = (a.get(i), b.get(i)) else {
                    return if a.get(i).is_none() { Ordering::Less } else { Ordering::Greater };
                };
                let order = match (is_numeric(x), is_numeric(y)) {
                    (true, true) => x.parse::<u64>().unwrap_or(0).cmp(&y.parse::<u64>().unwrap_or(0)),
                    (true, false) => Ordering::Less,
                    (false, true) => Ordering::Greater,
                    (false, false) => x.cmp(y),
                };
                if order != Ordering::Equal {
                    return order;
                }
            }
            Ordering::Equal
        }
    }
}

/// None if either side isn't a version.
pub fn compare_versions(a: &str, b: &str) -> Option<Ordering> {
    let (left, right) = (parse_version(a)?, parse_version(b)?);
    Some(left.core.cmp(&right.core).then_with(|| compare_prerelease(&left.pre, &right.pre)))
}

/// True when `last_check` (ms since epoch, 0 = never) is a day or more ago; a clock that went backwards counts as due.
pub fn daily_check_due(last_check: u64, now: u64) -> bool {
    last_check > now || now - last_check >= DAY_MS
}

pub fn should_auto_check(prefs: &Prefs, now: u64) -> bool {
    prefs.check_app_updates && daily_check_due(prefs.last_update_check, now)
}

pub fn should_auto_check_ytdlp(prefs: &Prefs, now: u64) -> bool {
    prefs.check_ytdlp_on_startup && daily_check_due(prefs.last_ytdlp_check, now)
}

pub fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64)
}

fn strip_separator<'a>(text: &'a str, allowed: &[char]) -> Option<&'a str> {
    let mut chars = text.chars();
    chars.next().filter(|c| allowed.contains(c))?;
    Some(chars.as_str())
}

/// What follows the product name and one separator in a download's file name: "PyxeeRadio" (the name
/// since the move to Tauri) or the older "Twitch Radio" in any of its spellings.
fn after_product_name<'a>(name: &'a str, allowed: &[char]) -> Option<&'a str> {
    if let Some(rest) = name.strip_prefix("PyxeeRadio") {
        return strip_separator(rest, allowed);
    }
    let rest = strip_separator(name.strip_prefix("Twitch")?, allowed)?;
    strip_separator(rest.strip_prefix("Radio")?, allowed)
}

/// GitHub turns spaces in asset names into dots, so both spellings are accepted:
/// "PyxeeRadio Setup 1.2.0.exe" and "PyxeeRadio.Setup.1.2.0.exe".
fn is_windows_installer(name: &str) -> bool {
    after_product_name(name, &[' ', '.'])
        .and_then(|rest| rest.strip_prefix("Setup"))
        .and_then(|rest| strip_separator(rest, &[' ', '.']))
        .is_some()
        && name.ends_with(".exe")
}

fn is_linux_download(name: &str, suffix: &str) -> bool {
    after_product_name(name, &[' ', '.', '-']).is_some() && name.ends_with(suffix)
}

/// (the release page, a direct download for this platform if there is one). Only links on the repo are used.
pub fn pick_links(release: &Value, repo: &str, platform: &str) -> (String, Option<String>) {
    let prefix = format!("https://github.com/{repo}/");
    let page = match release.get("html_url").and_then(Value::as_str) {
        Some(url) if url.starts_with(&prefix) => url.to_string(),
        _ => format!("{prefix}releases/latest"),
    };
    let assets: Vec<(&str, &str)> = release
        .get("assets")
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .filter_map(|a| Some((a.get("name")?.as_str()?, a.get("browser_download_url")?.as_str()?)))
                .filter(|(_, url)| url.starts_with(&prefix))
                .collect()
        })
        .unwrap_or_default();
    let find = |wanted: &dyn Fn(&str) -> bool| assets.iter().find(|(name, _)| wanted(name)).map(|(_, url)| url.to_string());
    let installer = match platform {
        "windows" => find(&is_windows_installer),
        // A portable archive and an AppImage are the same kind of download; the AppImage is the easier one.
        "linux" => find(&|n| is_linux_download(n, "linux-x64.AppImage")).or_else(|| find(&|n| is_linux_download(n, "linux-x64.tar.gz"))),
        _ => None,
    };
    (page, installer)
}

#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UpdateResult {
    pub ok: bool,
    pub current: String,
    pub latest: Option<String>,
    pub update_available: bool,
    pub release_url: Option<String>,
    pub installer_url: Option<String>,
    pub error: Option<String>,
}

/// Whatever fetches `(HTTP status, JSON body)` for a URL: `github_fetch`, or a stand-in in the tests.
pub type Fetch<'a> = dyn Fn(&str) -> Result<(u16, Value), String> + 'a;

/// Does the check with the given fetcher.
pub fn check_for_app_update(current: &str, platform: &str, repo: &str, fetch: &Fetch) -> UpdateResult {
    let mut result = UpdateResult {
        ok: false,
        current: current.to_string(),
        latest: None,
        update_available: false,
        release_url: None,
        installer_url: None,
        error: None,
    };
    let outcome = (|| -> Result<(), String> {
        let (status, release) = fetch(&format!("https://api.github.com/repos/{repo}/releases/latest"))?;
        if status == 404 {
            return Err("No release has been published yet.".into());
        }
        if !(200..300).contains(&status) {
            return Err(format!("GitHub answered {status}."));
        }
        let tag = release.get("tag_name").and_then(Value::as_str).unwrap_or("");
        let order = compare_versions(tag, current).ok_or_else(|| format!("Couldn't read the version \"{tag}\"."))?;
        let (page, installer) = pick_links(&release, repo, platform);
        result.ok = true;
        result.latest = Some(tag.strip_prefix('v').unwrap_or(tag).to_string());
        result.update_available = order == Ordering::Greater;
        result.release_url = Some(page);
        result.installer_url = installer;
        Ok(())
    })();
    if let Err(error) = outcome {
        result.error = Some(error);
    }
    result
}

/// The real fetch: GitHub's API over HTTPS, 15 seconds at most.
pub fn github_fetch(url: &str) -> Result<(u16, Value), String> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(15)))
        .http_status_as_error(false)
        .build()
        .into();
    let mut response = agent
        .get(url)
        .header("Accept", "application/vnd.github+json")
        .header("User-Agent", "TwitchRadio-update-check")
        .call()
        .map_err(|e| e.to_string())?;
    let status = response.status().as_u16();
    let body: Value = response.body_mut().read_json().unwrap_or(Value::Null);
    Ok((status, body))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn ordering(a: &str, b: &str) -> Option<i8> {
        compare_versions(a, b).map(|o| o as i8)
    }

    #[test]
    fn compare_versions_orders_releases_and_pre_releases() {
        assert_eq!(ordering("v1.1.0", "1.0.9"), Some(1));
        assert_eq!(ordering("1.0.0", "v1.0.0"), Some(0));
        assert_eq!(ordering("1.10.0", "1.9.0"), Some(1));
        assert_eq!(ordering("1.1.0-rc1", "1.1.0"), Some(-1));
        assert_eq!(ordering("1.1.0-rc1", "1.1.0-rc2"), Some(-1));
        assert_eq!(ordering("1.1.0-rc.2", "1.1.0-rc.10"), Some(-1));
        assert_eq!(ordering("1.1.0-1", "1.1.0-alpha"), Some(-1)); // numbers sort before words
        assert_eq!(ordering("1.1.0-rc", "1.1.0-rc.1"), Some(-1)); // a shorter list is lower
        assert_eq!(ordering("1.1.0+build5", "1.1.0"), Some(0));
        for bad in ["nonsense", "", "1.2", "1.2.3.4", "1.2.x", "1.2.3-", "1.2.3+"] {
            assert_eq!(ordering(bad, "1.0.0"), None, "{bad}");
        }
    }

    #[test]
    fn one_automatic_check_a_day_and_the_pref_is_honoured() {
        let now = 10 * DAY_MS;
        let prefs = |on, last| Prefs { check_app_updates: on, last_update_check: last, ..Prefs::default() };
        assert!(!should_auto_check(&prefs(false, 0), now));
        assert!(should_auto_check(&prefs(true, 0), now));
        assert!(!should_auto_check(&prefs(true, now - 1000), now));
        assert!(should_auto_check(&prefs(true, now - DAY_MS), now));
        assert!(should_auto_check(&prefs(true, now + 5000), now));
    }

    #[test]
    fn the_ytdlp_check_is_also_daily_with_its_own_pref_and_clock() {
        let now = 10 * DAY_MS;
        let prefs = |on, last| Prefs { check_ytdlp_on_startup: on, last_ytdlp_check: last, ..Prefs::default() };
        assert!(!should_auto_check_ytdlp(&prefs(false, 0), now));
        assert!(should_auto_check_ytdlp(&prefs(true, 0), now));
        assert!(!should_auto_check_ytdlp(&prefs(true, now - 1000), now));
        let both = Prefs { last_update_check: now, last_ytdlp_check: 0, ..Prefs::default() };
        assert!(should_auto_check_ytdlp(&both, now), "the two daily checks do not share a clock");
    }

    fn reply(status: u16, body: Value) -> impl Fn(&str) -> Result<(u16, Value), String> {
        move |_| Ok((status, body.clone()))
    }

    const BASE: &str = "https://github.com/Pylxyr/Twitch-Radio/releases/download/v1.2.0/";

    fn asset(name: &str) -> Value {
        json!({ "name": name, "browser_download_url": format!("{BASE}{name}") })
    }

    #[test]
    fn a_newer_release_is_reported_with_links_on_the_repo_only() {
        let body = json!({
            "tag_name": "v1.2.0",
            "html_url": "https://github.com/Pylxyr/Twitch-Radio/releases/tag/v1.2.0",
            "assets": [asset("SHA256SUMS.txt"), asset("PyxeeRadio.Setup.1.2.0.exe")],
        });
        let result = check_for_app_update("1.0.0", "windows", UPDATE_REPO, &reply(200, body));
        assert!(result.ok && result.update_available);
        assert_eq!(result.latest.as_deref(), Some("1.2.0"));
        assert!(result.installer_url.unwrap().contains("Setup"));
    }

    #[test]
    fn linux_gets_the_appimage_or_else_the_archive_and_windows_the_installer() {
        let body = json!({
            "tag_name": "v1.2.0",
            "html_url": "https://github.com/Pylxyr/Twitch-Radio/releases/tag/v1.2.0",
            "assets": [
                asset("PyxeeRadio.Setup.1.2.0.exe"),
                asset("PyxeeRadio-1.2.0-linux-x64.tar.gz"),
                asset("PyxeeRadio-1.2.0-linux-x64.AppImage"),
            ],
        });
        let linux = check_for_app_update("1.0.0", "linux", UPDATE_REPO, &reply(200, body.clone()));
        assert!(linux.installer_url.unwrap().ends_with("linux-x64.AppImage"));
        let win = check_for_app_update("1.0.0", "windows", UPDATE_REPO, &reply(200, body.clone()));
        assert!(win.installer_url.unwrap().ends_with("Setup.1.2.0.exe"));
        let mac = check_for_app_update("1.0.0", "macos", UPDATE_REPO, &reply(200, body));
        assert_eq!(mac.installer_url, None);
        assert!(mac.release_url.unwrap().contains("releases/tag"));

        let archive_only = json!({ "tag_name": "v1.2.0", "assets": [asset("PyxeeRadio-1.2.0-linux-x64.tar.gz")] });
        let linux = check_for_app_update("1.0.0", "linux", UPDATE_REPO, &reply(200, archive_only));
        assert!(linux.installer_url.unwrap().ends_with("linux-x64.tar.gz"));
    }

    #[test]
    fn downloads_published_under_the_old_product_name_are_still_found() {
        let body = json!({
            "tag_name": "v1.2.0",
            "assets": [asset("Twitch.Radio.Setup.1.2.0.exe"), asset("Twitch-Radio-1.2.0-linux-x64.tar.gz")],
        });
        let win = check_for_app_update("1.0.0", "windows", UPDATE_REPO, &reply(200, body.clone()));
        assert!(win.installer_url.unwrap().ends_with("Twitch.Radio.Setup.1.2.0.exe"));
        let linux = check_for_app_update("1.0.0", "linux", UPDATE_REPO, &reply(200, body));
        assert!(linux.installer_url.unwrap().ends_with("Twitch-Radio-1.2.0-linux-x64.tar.gz"));
    }

    #[test]
    fn other_files_are_not_mistaken_for_downloads() {
        for name in ["SHA256SUMS.txt", "PyxeeRadio.Setup.1.2.0.exe.sig", "PyxeeRadioSetup.exe", "Other.Setup.1.2.0.exe", "PyxeeRadio.1.2.0.exe"] {
            assert!(!is_windows_installer(name), "{name}");
        }
        for name in ["PyxeeRadio-1.2.0-win-x64.zip", "Other-1.2.0-linux-x64.tar.gz", "PyxeeRadio-1.2.0-linux-arm64.tar.gz"] {
            assert!(!is_linux_download(name, "linux-x64.tar.gz"), "{name}");
        }
    }

    #[test]
    fn foreign_urls_are_ignored_and_errors_are_reported() {
        let evil = json!({
            "tag_name": "v9.0.0",
            "html_url": "https://evil.example/x",
            "assets": [{ "name": "PyxeeRadio Setup 9.0.0.exe", "browser_download_url": "https://evil.example/a.exe" }],
        });
        let result = check_for_app_update("1.0.0", "windows", UPDATE_REPO, &reply(200, evil));
        assert_eq!(result.installer_url, None);
        assert!(result.release_url.unwrap().starts_with("https://github.com/Pylxyr/Twitch-Radio/"));

        let up_to_date = check_for_app_update("1.2.0", "linux", UPDATE_REPO, &reply(200, json!({ "tag_name": "v1.2.0" })));
        assert!(up_to_date.ok && !up_to_date.update_available);
        assert!(!check_for_app_update("1.0.0", "linux", UPDATE_REPO, &reply(404, json!({}))).ok);
        assert!(!check_for_app_update("1.0.0", "linux", UPDATE_REPO, &reply(500, json!({}))).ok);
        let offline = check_for_app_update("1.0.0", "linux", UPDATE_REPO, &|_| Err("offline".into()));
        assert_eq!(offline.error.as_deref(), Some("offline"));
        let odd_tag = check_for_app_update("1.0.0", "linux", UPDATE_REPO, &reply(200, json!({ "tag_name": "nightly" })));
        assert!(odd_tag.error.unwrap().contains("nightly"));
    }
}
