//! Validate receipt metadata without granting the writer any static uninstall authority.
use super::{json, receipt::BINARY};
use serde_json::Value;
use std::collections::HashSet;

fn relative(s: &str) -> bool {
    !s.is_empty()
        && !s.starts_with('/')
        && !s.contains('\0')
        && s.split('/').all(|p| !p.is_empty() && p != "." && p != "..")
}
fn owned_static(s: &str) -> bool {
    if s == BINARY || s == "layouts/panorama.json" {
        return true;
    }
    let mut p = s.split('/');
    p.next() == Some("mods")
        && p.next()
            .is_some_and(|m| m == "panorama" || m.starts_with("panorama-"))
        && p.next().is_some()
}
fn mode(v: &Value) -> bool {
    v.as_i64().is_some_and(|m| (0..=0o777).contains(&m))
}
pub fn validate(v: &Value) -> Result<(), String> {
    let files = v["files"].as_object().ok_or("invalid receipt files")?;
    for (name, info) in files {
        if !relative(name) || !owned_static(name) {
            return Err("receipt file inventory escapes static panorama payload".into());
        }
        json::keys(info, &["sha256", "mode"])?;
        if !info["sha256"].as_str().is_some_and(|s| json::hex(s, 64))
            || info["mode"].as_i64() != Some(if name == BINARY { 0o755 } else { 0o644 })
        {
            return Err("invalid static fingerprint".into());
        }
    }
    for name in [
        "panorama",
        "panorama-spaces",
        "panorama-graph",
        "panorama-peek",
    ] {
        for file in ["main.js", "mod.json"] {
            if !files.contains_key(&format!("mods/{name}/{file}")) {
                return Err("receipt static package is incomplete".into());
            }
        }
    }
    for name in [
        "layouts/panorama.json",
        "mods/panorama/LICENSE.herdr-app",
        "mods/panorama/NOTICE",
    ] {
        if !files.contains_key(name) {
            return Err("receipt static package is incomplete".into());
        }
    }
    let dirs = v["dirs"].as_array().ok_or("invalid receipt directories")?;
    let mut seen = HashSet::new();
    for dir in dirs {
        let path = dir
            .as_str()
            .filter(|s| relative(s))
            .ok_or("unsafe receipt directory")?;
        if !seen.insert(path) {
            return Err("duplicate receipt directory".into());
        }
    }
    let keys = &v["keys"];
    if !keys.is_null() {
        json::keys(
            keys,
            &[
                "before",
                "before_mode",
                "after",
                "mode",
                "added",
                "sections",
            ],
        )?;
        if !(keys["before"].is_null() || keys["before"].is_string())
            || !keys["after"].is_string()
            || !mode(&keys["mode"])
            || !keys["added"].is_object()
            || !keys["sections"]
                .as_array()
                .is_some_and(|s| s.iter().all(Value::is_string))
        {
            return Err("invalid receipt key journal".into());
        }
        if (keys["before"].is_null() && !keys["before_mode"].is_null())
            || (!keys["before"].is_null() && !mode(&keys["before_mode"]))
        {
            return Err("invalid original key mode".into());
        }
    }
    Ok(())
}
