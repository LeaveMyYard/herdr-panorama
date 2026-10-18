//! rsync filter rules for exactly the files the scanner reads of some sessions (Python
//! `machines._rules`).

use crate::machines::valid_session;

/// Which pass a rule set is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Append-only files, copied with `--append-verify` when rsync has it: the transcript,
    /// agent transcripts, workflow journals.
    Jsonl,
    /// Files rewritten in place (maybe shorter, maybe at the same size): agent metas,
    /// workflow run summaries.
    Json,
}

/// Every directory, then each valid session's files of this kind, then nothing else. An id
/// that is not a session id adds no rule.
pub fn rules(sessions: &[String], kind: Kind) -> Vec<String> {
    let mut out = vec!["--include=*/".to_owned()];
    for sid in sessions.iter().filter(|s| valid_session(s)) {
        let patterns: &[&str] = match kind {
            Kind::Jsonl => &[
                "*/{sid}.jsonl",
                "*/{sid}/subagents/agent-*.jsonl",
                "*/{sid}/subagents/workflows/*/journal.jsonl",
                "*/{sid}/subagents/workflows/*/agent-*.jsonl",
            ],
            Kind::Json => &[
                "*/{sid}/subagents/agent-*.meta.json",
                "*/{sid}/subagents/workflows/*/agent-*.meta.json",
                "*/{sid}/workflows/*.json",
            ],
        };
        out.extend(
            patterns
                .iter()
                .map(|p| format!("--include={}", p.replace("{sid}", sid))),
        );
    }
    out.push("--exclude=*".to_owned());
    out
}

#[cfg(test)]
mod tests;
