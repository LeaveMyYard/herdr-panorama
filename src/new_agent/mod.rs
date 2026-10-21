//! The + New agent modal and the start behind it (Python `home.slugify`, `home.new_agent`,
//! `tui.NewAgentForm`; design 1C, approved deviation 2026-10-17: no first prompt, nothing is
//! ever typed into the pane). SAFETY: it creates a tab, starts an agent in it, and closes that
//! tab again on a failed start.
//!
//! - [`form`]: the modal's state: the harness carousel, the two fields, the status row, the
//!   arming; keys and clicks in, a start or a cancel out (no IO);
//! - [`start`]: starting the agent through [`ops::StartOps`] ([`real`] is herdr itself).

pub mod form;
pub mod ops;
pub mod real;
pub mod start;
pub mod text;

/// The kind of agent panorama starts (the harnesses' `id`).
pub const KIND: &str = "claude";

/// A herdr agent name from free text (`[a-z][a-z0-9_-]{0,31}`); "" when nothing is left.
pub fn slugify(name: &str) -> String {
    let lower = name.trim().to_lowercase();
    let mut s = String::new();
    let mut gap = false;
    for c in lower.chars() {
        if c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-' {
            if gap {
                s.push('-');
                gap = false;
            }
            s.push(c);
        } else {
            gap = true;
        }
    }
    let mut s = s.trim_matches(['-', '_']).to_owned();
    while s.contains("--") {
        s = s.replace("--", "-");
    }
    if s.chars().next().is_some_and(|c| !c.is_ascii_lowercase()) {
        s = format!("a-{s}");
    }
    let cut: String = s.chars().take(32).collect();
    cut.trim_end_matches(['-', '_']).to_owned()
}

/// herdr's agent-name rule.
pub fn name_ok(slug: &str) -> bool {
    let mut cs = slug.chars();
    cs.next().is_some_and(|c| c.is_ascii_lowercase())
        && slug.len() <= 32
        && cs.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
}

#[cfg(test)]
mod tests;
