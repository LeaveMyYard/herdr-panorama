//! Titles (plan `uniform-agent-tree.md` §2.2): what the views call an agent or a root, and
//! which rule gave it. Every title is cleaned first ([`clean`]); a candidate that cleans to
//! nothing is skipped. Claude's rules live here; Codex has its own (`collect::codex`).

use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::path::Path;

/// The longest title stored; views truncate further.
pub const MAX: usize = 80;
/// The most of a transcript's first line read for its prompt.
const FIRST_LINE: u64 = 256 * 1024;

/// Control and bidi characters dropped, whitespace collapsed, leading decorations (anything
/// before the first letter or digit, `model.js`'s rule) stripped, cut to [`MAX`] characters.
pub fn clean(raw: &str) -> String {
    let kept: String = raw
        .chars()
        .map(|c| if c.is_whitespace() { ' ' } else { c })
        .filter(|c| !c.is_control() && !is_bidi(*c))
        .collect();
    let words = kept.split(' ').filter(|w| !w.is_empty());
    let joined = words.collect::<Vec<_>>().join(" ");
    let start = joined.trim_start_matches(|c: char| !c.is_alphanumeric());
    if start.chars().count() <= MAX {
        return start.to_owned();
    }
    let cut: String = start.chars().take(MAX - 1).collect();
    format!("{}…", cut.trim_end())
}

fn is_bidi(c: char) -> bool {
    matches!(c, '\u{061C}' | '\u{200E}' | '\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}')
}

/// The first candidate that cleans to something, with its rule's name.
pub fn first<'a, I>(candidates: I) -> Option<(String, &'static str)>
where
    I: IntoIterator<Item = (Option<&'a str>, &'static str)>,
{
    candidates.into_iter().find_map(|(text, source)| {
        let t = clean(text?);
        (!t.is_empty()).then_some((t, source))
    })
}

/// A Claude subagent's title: its `meta.json` description, the spawn's description, the
/// first line of its first prompt, its type and short id, else `agent <short id>`.
pub fn claude_agent(
    meta_description: Option<&str>,
    spawn_description: Option<&str>,
    prompt: impl FnOnce() -> Option<String>,
    agent_type: Option<&str>,
    id: &str,
) -> (String, &'static str) {
    let short: String = id.chars().take(7).collect();
    if let Some(t) = first([
        (meta_description, "spawner"),
        (spawn_description, "spawner"),
    ]) {
        return t;
    }
    let prompt = prompt();
    let typed = agent_type.map(|t| format!("{t} {short}"));
    first([(prompt.as_deref(), "prompt"), (typed.as_deref(), "type")])
        .unwrap_or_else(|| (format!("agent {short}"), "id"))
}

/// The first line of the prompt a subagent's transcript opens with (its first record, a
/// `user` message with text content); `None` when it opens otherwise or cannot be read.
pub fn first_prompt(transcript: &Path) -> Option<String> {
    let file = File::open(transcript).ok()?;
    let mut line = String::new();
    BufReader::new(file.take(FIRST_LINE))
        .read_line(&mut line)
        .ok()?;
    let rec: serde_json::Value = serde_json::from_str(line.trim_end()).ok()?;
    if rec.get("type")?.as_str()? != "user" {
        return None;
    }
    let content = rec.get("message")?.get("content")?;
    let text = match content {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Array(blocks) => blocks
            .iter()
            .filter(|b| b.get("type").and_then(|t| t.as_str()) == Some("text"))
            .find_map(|b| b.get("text").and_then(|t| t.as_str()).map(str::to_owned))?,
        _ => return None,
    };
    first_line(&text)
}

/// The first non-blank line.
pub fn first_line(text: &str) -> Option<String> {
    text.lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .map(str::to_owned)
}

/// A herdr label the operator did not choose: empty, all digits (herdr's numbering), the
/// agent's kind or program name, `tab`, or one Panorama writes back itself when it leaves a
/// tab (`home::label::neutral_label`: the agent's herdr name, else its kind, else `tab`).
pub fn is_neutral(label: &str, kind: &str, herdr_name: Option<&str>) -> bool {
    let l = clean(label).to_lowercase();
    l.is_empty()
        || l.chars().all(|c| c.is_ascii_digit())
        || ["claude", "codex", "tab", "claude code"].contains(&l.as_str())
        || l == kind.to_lowercase()
        || herdr_name.is_some_and(|n| clean(n).to_lowercase() == l)
}

/// A terminal title worth showing (`model.js`'s "telling" rule): not a path, not
/// `user@host`, not the program's name.
pub fn is_telling(title: &str, kind: &str) -> bool {
    let t = clean(title);
    let raw = title.trim_start();
    let path = raw.starts_with(['/', '~']) || raw.as_bytes().get(1..3) == Some(b":\\".as_slice());
    !t.is_empty() && !path && !t.contains('@') && !is_neutral(&t, kind, None)
}

/// What a Claude root's title is chosen from (§2.2 "Root, Claude").
pub struct RootInputs<'a> {
    pub kind: &'a str,
    /// The agent's herdr name (`agents[].name`), the pane's name.
    pub pane_name: Option<&'a str>,
    /// Its tab's label, the inbox mark already stripped.
    pub tab_label: &'a str,
    /// Agents in the tab: a tab label names a root only when it holds just this one (D6).
    pub per_tab: usize,
    /// `custom-title`, `agent-name`, `ai-title`, in that order.
    pub session: [Option<&'a str>; 3],
    pub first_prompt: Option<&'a str>,
    pub terminal_title: Option<&'a str>,
}

/// A root's title from the operator's herdr names, its session and its terminal; `None`
/// when none gives one ([`root_fallback`] then names it by its tab or pane).
pub fn root(r: &RootInputs) -> Option<(String, &'static str)> {
    let pane = r.pane_name.filter(|n| !is_neutral(n, r.kind, None));
    let tab =
        (r.per_tab <= 1 && !is_neutral(r.tab_label, r.kind, r.pane_name)).then_some(r.tab_label);
    let terminal = r.terminal_title.filter(|t| is_telling(t, r.kind));
    let [custom, agent, ai] = r.session;
    first([
        (pane, "pane"),
        (tab, "tab"),
        (custom, "session"),
        (agent, "session"),
        (ai, "session"),
        (r.first_prompt, "prompt"),
        (terminal, "pane"),
    ])
}

/// The last rung: `Claude · tab N`, else `Claude · <pane>`.
pub fn root_fallback(harness: &str, tab_no: Option<i64>, pane: &str) -> (String, &'static str) {
    let mut chars = harness.chars();
    let name = chars.next().map_or_else(
        || "Agent".to_owned(),
        |c| c.to_uppercase().chain(chars).collect(),
    );
    let at = tab_no.map_or_else(|| pane.to_owned(), |n| format!("tab {n}"));
    (format!("{name} · {at}"), "id")
}

#[cfg(test)]
mod tests;
