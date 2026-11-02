//! Harness identity in the existing title slot. IO capability detection belongs to the
//! binary; the core only consumes an explicit, deterministic presentation mode.

use super::new_agent::HARNESSES;
use crate::model::Orchestrator;

/// Herdr Agent Icons Max vendor glyphs (herdr-radar tools/codepoints.toml).
pub const CLAUDE: char = '\u{e1a0}';
pub const CODEX: char = '\u{e1a1}';

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum HarnessIcons {
    #[default]
    Text,
    Font,
}

pub fn title(o: &Orchestrator, icons: HarnessIcons) -> String {
    let icon = match (icons, o.harness.as_str()) {
        (HarnessIcons::Font, "claude") => Some(CLAUDE),
        (HarnessIcons::Font, "codex") => Some(CODEX),
        _ => None,
    };
    if let Some(icon) = icon {
        return format!("{icon} {}", o.tab_label);
    }
    let name = HARNESSES
        .iter()
        .find(|h| h.id == o.harness)
        .map_or(o.harness.as_str(), |h| h.name);
    format!("{name} · {}", o.tab_label)
}
