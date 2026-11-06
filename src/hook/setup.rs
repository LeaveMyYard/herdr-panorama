//! `panorama hook setup`: print, never apply, the Claude Code hooks to merge into user settings.
use serde_json::{Value, json};

fn entry(matcher: &str, command: String) -> Value {
    json!([{ "matcher": matcher, "hooks": [{ "type": "command", "command": command, "timeout": 10 }] }])
}

/// The `{"hooks": …}` object for `binary` (an absolute path). The binary is quoted for `sh`;
/// Claude Code runs a command hook's string through the shell.
pub fn snippet(binary: &str) -> Value {
    let quoted = format!("'{}'", binary.replace('\'', r"'\''"));
    // Like the shipped hooks file: silent and exit 0 even if the binary moves later.
    let run = |kind: &str| format!("{quoted} hook {kind} >/dev/null 2>&1; exit 0");
    json!({ "hooks": {
        "SessionStart": entry("startup|resume|clear|compact|fork", run("session-start")),
        "SubagentStart": entry(".+", run("subagent-start")),
        "SubagentStop": entry(".+", run("subagent-stop")),
    }})
}

/// Prints the snippet and where it goes; writes no file. 0, or 1 without an absolute binary.
pub fn main() -> i32 {
    let Some(binary) = std::env::current_exe()
        .ok()
        .filter(|p| p.is_absolute())
        .and_then(|p| p.to_str().map(str::to_owned))
    else {
        eprintln!("panorama hook setup: cannot name this binary's absolute path");
        return 1;
    };
    eprintln!(
        "Merge these entries into the \"hooks\" object of ~/.claude/settings.json \
         (or $CLAUDE_CONFIG_DIR/settings.json), keeping your existing hooks. Panorama does not \
         edit it. New, resumed and cleared sessions then record their terminal and process."
    );
    println!(
        "{}",
        serde_json::to_string_pretty(&snippet(&binary)).unwrap_or_default()
    );
    0
}
