//! The home record (Python `home._load`/`_save`): `home-<session>.json` in the state dir,
//! the last home pane and tab. Only a hint: the home is always found by content.

use super::Home;
use serde_json::{Value, json};
use std::path::Path;

/// The recorded home pane, if any can be read.
pub fn load_pane(path: &Path) -> Option<String> {
    let v: Value = serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()?;
    v.get("pane_id").and_then(Value::as_str).map(str::to_owned)
}

/// Writes the record whole, through a temporary file; why not, if it failed.
pub fn save(path: &Path, home: &Home) -> Result<(), String> {
    let write = || -> std::io::Result<()> {
        if let Some(d) = path.parent() {
            std::fs::create_dir_all(d)?;
        }
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, json!({"pane_id": home.pane_id, "tab_id": home.tab_id}).to_string())?;
        std::fs::rename(&tmp, path)
    };
    write().map_err(|e| format!("could not save the home record: {e}"))
}
