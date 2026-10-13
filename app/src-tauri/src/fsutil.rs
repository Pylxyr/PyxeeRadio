//! Small file helpers shared by the modules that keep settings on disk.

use serde_json::{Map, Value};
use std::path::Path;

/// Writes through a temporary file and renames it over the target, so a crash never leaves half a file.
pub fn atomic_write(file: &Path, text: &str) -> std::io::Result<()> {
    if let Some(parent) = file.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = file.with_extension(format!("{}.{}.tmp", file.extension().and_then(|e| e.to_str()).unwrap_or(""), std::process::id()));
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, file).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })
}

/// A JSON object from a file; anything else (missing, unreadable, not an object) is an empty one.
pub fn read_json_object(file: &Path) -> Map<String, Value> {
    std::fs::read_to_string(file)
        .ok()
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        .and_then(|value| match value {
            Value::Object(map) => Some(map),
            _ => None,
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atomic_write_replaces_the_file_and_leaves_no_temp_behind() {
        let dir = crate::prefs::tests::tempdir("fsutil");
        let file = dir.join("a").join("x.json");
        atomic_write(&file, "{\"a\":1}").unwrap();
        atomic_write(&file, "{\"a\":2}").unwrap();
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "{\"a\":2}");
        let names: Vec<_> = std::fs::read_dir(file.parent().unwrap()).unwrap().map(|e| e.unwrap().file_name()).collect();
        assert_eq!(names.len(), 1, "{names:?}");
    }

    #[test]
    fn read_json_object_is_empty_for_anything_else() {
        let dir = crate::prefs::tests::tempdir("fsutil-read");
        std::fs::write(dir.join("list.json"), "[1,2]").unwrap();
        std::fs::write(dir.join("ok.json"), "{\"k\": 1}").unwrap();
        assert!(read_json_object(&dir.join("list.json")).is_empty());
        assert!(read_json_object(&dir.join("missing.json")).is_empty());
        assert_eq!(read_json_object(&dir.join("ok.json")).get("k"), Some(&Value::from(1)));
    }
}
