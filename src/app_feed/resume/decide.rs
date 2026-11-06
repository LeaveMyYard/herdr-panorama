//! The rules, with no IO: was this a loss, may this terminal be typed into, did it work.
use super::{
    hc::{Lookup, Status, Term},
    launch::launch,
    state::{Entry, Launch, Loss, Record, Saved, Stage},
};
use crate::host::source::Linked;
use std::{collections::BTreeMap, mem::take};

/// A missing agent is called quit only after this long with the core still up. The core's
/// own stop hangs up, then terminates, then kills, over 3 s, and answers `hc` meanwhile.
pub const ENDING_MS: u64 = 4_000;
/// After the first drawn screen, it must stay exactly that for this long.
pub const SETTLE_MS: u64 = 1_500;
/// Give-ups. Each ends in not typing, so none of them can cause a wrong resume.
pub const SHELL_WAIT_MS: u64 = 120_000;
pub const CONFIRM_MS: u64 = 30_000;
/// A core up longer than this before Panorama saw it may have had its shells used already.
pub const FRESH_CORE_MS: u64 = 300_000;

/// Shells whose restored prompt was tested. Any other is never typed into.
const SHELLS: [&str; 3] = ["bash", "zsh", "fish"];

#[derive(Debug, PartialEq, Eq)]
pub enum Change {
    Same,
    /// A restarted core has not started its holder yet: nothing can be said.
    NoHolderYet,
    /// Every holder that ran the recorded terminals is gone.
    Lost,
    /// The core's state was wiped: addresses mean nothing any more.
    Foreign,
}

/// Holders run the programs. `hc server restart`, a core crash and an app restart all keep
/// them, and so the same instance ids. Only a real loss replaces every one.
pub fn compare(saved: &Saved, status: &Status) -> Change {
    match &saved.core {
        Some(core) if *core != status.core => Change::Foreign,
        Some(_) if !saved.holders.is_empty() && status.holders.is_empty() => Change::NoHolderYet,
        Some(_)
            if !saved.holders.is_empty()
                && !saved.holders.iter().any(|h| status.holders.contains(h)) =>
        {
            Change::Lost
        }
        _ => Change::Same,
    }
}

/// A proved link as a record, or `None` when it has nothing a resume could use.
pub fn record(link: &Linked) -> Option<Record> {
    let term = Term::parse(&link.terminal)?;
    let foreground = term.foreground?;
    if link.address.contains('/') || !matches!(link.harness.as_str(), "claude" | "codex") {
        return None;
    }
    let argv: Vec<String> = link.terminal["foreground"]["argv"]
        .as_array()?
        .iter()
        .filter_map(|a| a.as_str().map(str::to_owned))
        .collect();
    let (launch, dropped, no_daemon) = launch(&link.harness, &argv);
    let record = Record {
        address: link.address.clone(),
        harness: link.harness.clone(),
        session: link.session.clone(),
        cwd: foreground.cwd,
        root_pid: term.pid?,
        agent_pid: foreground.pid,
        created_ms: term.created_ms,
        launch,
        dropped,
        no_daemon,
        default_config: link.default_config,
        proved: true,
        ending_ms: None,
    };
    record.command().map(|_| record)
}

/// `old` after a look at its terminal, or `None` for a session that is over.
fn still(mut old: Record, term: &Term, now_ms: u64) -> Option<Record> {
    if term.created_ms != old.created_ms {
        return None;
    }
    match term.reason.as_deref() {
        Some("exited" | "killed") => return None,
        // Lost, and the core says so: the session did not end by anyone's choice.
        Some(_) => return Some(old),
        None if term.pid.is_none() => return Some(old),
        None => {}
    }
    if term.foreground.as_ref().map(|f| f.pid) == Some(old.agent_pid) {
        old.proved = false;
        old.ending_ms = None;
        return Some(old);
    }
    match old.ending_ms {
        Some(at) if now_ms.saturating_sub(at) >= ENDING_MS => None,
        Some(_) => Some(old),
        None => {
            old.ending_ms = Some(now_ms);
            Some(old)
        }
    }
}

/// The records after a collect. Only a positive read ever ends one: a failed read says nothing.
pub fn track(saved: &mut Saved, linked: &[Linked], get: &dyn Fn(&str) -> Lookup, now_ms: u64) {
    let fresh: BTreeMap<String, Record> = linked
        .iter()
        .filter_map(record)
        .map(|r| (r.address.clone(), r))
        .collect();
    let mut next = BTreeMap::new();
    for (address, old) in take(&mut saved.records) {
        if fresh.contains_key(&address) {
            continue;
        }
        let kept = match get(&address) {
            Lookup::Gone => None,
            Lookup::Unknown => Some(old),
            Lookup::Found(term) => still(old, &term, now_ms),
        };
        next.extend(kept.map(|r| (address, r)));
    }
    next.extend(fresh);
    saved.records = next;
}

/// Proved only by two ids that differ. A missing one proves nothing.
fn rebooted(saved: &Saved, boot: Option<&str>) -> bool {
    matches!((saved.boot.as_deref(), boot), (Some(old), Some(new)) if old != new)
}

/// Why this loss may only be resumed by a click. `None`: the terminal checks decide.
fn barred(
    saved: &Saved,
    record: &Record,
    uptime_ms: u64,
    boot: Option<&str>,
) -> Option<&'static str> {
    if saved.closed {
        Some("Panorama was closed, so it can't tell if this session was live")
    } else if !record.proved {
        Some("the session wasn't proved at the last look")
    } else if record.ending_ms.is_some() {
        Some("the agent had already gone just before the loss")
    } else if record.launch == Launch::Other {
        Some("the resume command can't repeat how it was started")
    } else if !record.default_config {
        Some("it ran under another config dir")
    } else if !rebooted(saved, boot) {
        // `hc server stop` and a crash look alike from here; a reboot doesn't.
        Some("stopped without a reboot, maybe on purpose")
    } else if uptime_ms > FRESH_CORE_MS {
        Some("the core was up too long before Panorama saw it")
    } else {
        None
    }
}

/// Moves every record to this loss. Anything still open from an earlier one is over, and
/// what that one could not resume stays listed: its command may be the only copy of the id.
pub fn open(saved: &mut Saved, status: &Status, boot: Option<&str>, now_ms: u64) {
    let earlier = saved.loss.as_mut().map(|l| &mut l.entries);
    for entry in earlier
        .into_iter()
        .flat_map(|e| e.values_mut())
        .filter(|e| e.open())
    {
        entry.end(Stage::Skipped, "another loss came first");
    }
    if saved.records.is_empty() {
        return;
    }
    let mut entries = saved.loss.take().map_or(BTreeMap::new(), |l| l.entries);
    entries.retain(|_, e| e.stage != Stage::Resumed);
    for (address, record) in take(&mut saved.records) {
        let hold = barred(saved, &record, status.uptime_ms, boot).map(str::to_owned);
        let mut entry = Entry::new(record);
        entry.hold = hold;
        entry.rebooted = rebooted(saved, boot);
        entries.insert(address, entry);
    }
    saved.loss = Some(Loss {
        detected_ms: now_ms,
        entries,
    });
}

#[derive(Debug, PartialEq, Eq)]
pub enum Verdict {
    /// Not decided yet: the shell has not come back.
    Wait,
    /// A program is in front of the shell. Startup can look like this too.
    Busy(&'static str),
    /// Not now, though a person could change that.
    Skip(&'static str),
    /// Never, whatever a person does.
    Dead(&'static str),
    Ready,
}

/// May this terminal be typed into? It must be a bare known shell, in the right folder, in
/// the terminal the record names. `rebooted`: pids from before mean nothing.
pub fn judge(record: &Record, term: &Term, rebooted: bool) -> Verdict {
    if !term.live {
        return Verdict::Dead("the terminal has finished");
    }
    if term.created_ms != record.created_ms {
        return Verdict::Dead("this address is a different terminal now");
    }
    if !term.keep {
        return Verdict::Dead("the terminal isn't kept after loss");
    }
    let (Some(pid), None) = (term.pid, &term.reason) else {
        return Verdict::Wait;
    };
    if pid == record.root_pid && !rebooted {
        return Verdict::Dead("the terminal's program was never replaced");
    }
    if term.reports {
        return Verdict::Busy("a program is reporting status in the terminal");
    }
    let Some(foreground) = &term.foreground else {
        return Verdict::Wait;
    };
    if foreground.pid != pid {
        return Verdict::Busy("something is running in the terminal");
    }
    // `exec python3` keeps the pid: only the words show it isn't the shell any more.
    if foreground.argv != term.argv || !SHELLS.contains(&foreground.name.as_str()) {
        return Verdict::Dead("the terminal's program isn't a shell Panorama knows");
    }
    if foreground.cwd != record.cwd {
        return Verdict::Skip("the shell is in a different folder");
    }
    Verdict::Ready
}

/// Read-only, after the one send. `link` is the session proved there now, if any.
pub fn confirm(entry: &mut Entry, lookup: &Lookup, link: Option<&str>, arrived: bool, now_ms: u64) {
    let overdue = now_ms >= entry.sent_ms.unwrap_or(0).saturating_add(CONFIRM_MS);
    if let Some(id) = link {
        entry.stage = Stage::Resumed;
        entry.reason = (id != entry.record.session).then(|| format!("now running session {id}"));
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
            tell("the command reached the terminal but nothing started; not retried"),
        ),
        _ if overdue => entry.end(Stage::Failed, tell("no sign it arrived; not retried")),
        _ => entry.reason = note,
    }
}
