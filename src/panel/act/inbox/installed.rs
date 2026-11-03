//! Constrained app navigation: existing installed CLI only, with no action fallback.
use super::*;
use serde::Deserialize;
use std::collections::HashSet;

/// The identity/availability projection of InstalledPluginInfo; extra metadata is additive.
#[derive(Deserialize)]
struct Registration {
    plugin_id: String,
    plugin_root: String,
    enabled: bool,
}
fn registrations(value: serde_json::Value) -> Result<Vec<Registration>, String> {
    let entries = value
        .as_object()
        .and_then(|o| o.get("plugins"))
        .and_then(serde_json::Value::as_array)
        .ok_or("malformed plugin lookup result: expected plugins array")?;
    // Serde can deserialize structs from positional arrays; the wire requires objects.
    if entries.iter().any(|entry| !entry.is_object()) {
        return Err("malformed plugin lookup result: expected registration object".into());
    }
    let entries: Vec<Registration> =
        serde_json::from_value(serde_json::Value::Array(entries.clone()))
            .map_err(|e| format!("malformed plugin lookup result: {e}"))?;
    let mut ids = HashSet::new();
    if entries
        .iter()
        .any(|p| p.plugin_id.is_empty() || !ids.insert(&p.plugin_id))
    {
        return Err("malformed plugin lookup result: empty or duplicate plugin id".into());
    }
    Ok(entries)
}

#[derive(Clone, Debug)]
pub struct InstalledInbox {
    root: String,
    pub questions: bool,
    no_hold: bool,
}
impl InboxLink {
    /// Only successful installed `open --help` proves this entrypoint available.
    pub async fn installed(
        &self,
        h: &Herdr,
        run: &impl Runner,
    ) -> Result<Option<InstalledInbox>, String> {
        // The ordinary TUI lookup intentionally treats failures as optional absence.
        // Strict app availability must retain transport/parse errors, without caching them.
        // The kit's typed list decoder intentionally skips malformed items for legacy UI.
        // Validate this raw projection before drawing any absence/disabled conclusion.
        let result = h
            .raw("plugin.list", serde_json::json!({}))
            .await
            .map_err(|e| format!("installed inbox plugin lookup failed: {e}"))?;
        let plugins = registrations(result)?;
        let Some(plugin) = plugins
            .into_iter()
            .find(|p| p.plugin_id == INBOX_ID && p.enabled)
        else {
            return Ok(None);
        };
        let root = plugin.plugin_root;
        if root.is_empty() {
            return Ok(None);
        }
        let result = run.inbox(&root, &["open", "--help"]).await;
        if result.code == 124 {
            return Err("installed inbox help timed out".into());
        }
        if result.code != 0 {
            return Ok(None);
        }
        let help = result.out;
        Ok(Some(InstalledInbox {
            root,
            questions: help.contains("--question"),
            no_hold: help.contains("--no-hold"),
        }))
    }
}
impl InstalledInbox {
    /// Returns only the installed CLI result. No retry or registered-action fallback.
    pub async fn open(&self, run: &impl Runner, question: Option<i64>) -> Result<(), String> {
        if question.is_some_and(|q| !(1..=9_007_199_254_740_991).contains(&q)) {
            return Err("invalid question id".into());
        }
        if question.is_some() && !self.questions {
            return Err("installed inbox has no question navigation".into());
        }
        let mut args = vec!["open".to_owned()];
        if let Some(q) = question {
            args.extend(["--question".into(), format!("Q{q}")]);
        }
        if self.no_hold {
            args.push("--no-hold".into());
        }
        let refs: Vec<_> = args.iter().map(String::as_str).collect();
        let result = run.inbox(&self.root, &refs).await;
        if result.code == 0 {
            Ok(())
        } else {
            Err(format!(
                "inbox navigation failed (exit {}); no retry",
                result.code
            ))
        }
    }
}
