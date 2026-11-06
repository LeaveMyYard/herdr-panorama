//! What became of a send. Read-only: nothing here types.
use super::{
    hc::Lookup,
    state::{Entry, Stage},
};

pub const CONFIRM_MS: u64 = 30_000;

/// Read-only, after the one send. `link` is the session proved there now, if any.
pub fn confirm(
    entry: &mut Entry,
    lookup: &Lookup,
    link: Option<(&str, bool)>,
    arrived: bool,
    now_ms: u64,
) {
    let overdue = now_ms >= entry.sent_ms.unwrap_or(0).saturating_add(CONFIRM_MS);
    if let Some((id, same_config)) = link {
        entry.stage = Stage::Resumed;
        entry.reason = if !same_config {
            Some("running under another config dir".into())
        } else {
            (id != entry.record.session).then(|| format!("now running session {id}"))
        };
        return;
    }
    let shell = match lookup {
        Lookup::Found(t) if t.live => match (&t.foreground, t.pid) {
            (Some(f), Some(pid)) if f.pid != pid => Some(false),
            (Some(_), Some(_)) => Some(true),
            _ => None,
        },
        _ => None,
    };
    let note = entry.reason.take();
    let tell = |text: &str| {
        note.as_ref()
            .map_or(text.to_owned(), |n| format!("{n}; {text}"))
    };
    match shell {
        Some(false) if overdue => {
            entry.end(Stage::Resumed, "started, but its session isn't proved")
        }
        Some(false) => {
            entry.started = true;
            entry.reason = note;
        }
        Some(true) if entry.started => entry.end(
            Stage::Failed,
            tell("it exited soon after starting; not retried"),
        ),
        _ if overdue && arrived => entry.end(
            Stage::Failed,
            tell("the command reached the terminal, but no session started; read it there; not retried"),
        ),
        _ if overdue => entry.end(Stage::Failed, tell("no session started, and a startup prompt may have taken the text; not retried")),
        _ => entry.reason = note,
    }
}
