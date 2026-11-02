//! A hook must belong to the selected pane's embedded Codex foreground process.
use herdr_agent_kit::agent_procs::{MAX_DEPTH, Pid, ProcessTable};
use herdr_agent_kit::herdr_api::{Herdr, PaneId, ProcessInfo};
use std::time::Duration;
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Owner {
    pub pid: Pid,
    pub start: String,
}
fn codex(argv: &[String]) -> bool {
    argv.first()
        .is_some_and(|arg| arg.rsplit('/').next() == Some("codex"))
        && !argv
            .iter()
            .take_while(|arg| arg.as_str() != "--")
            .any(|arg| matches!(arg.as_str(), "app-server" | "daemon"))
}
pub(super) fn embedded(argv: &[String]) -> bool {
    codex(argv) && argv.get(1).is_some_and(|arg| arg == "--no-daemon")
}
pub(super) fn ancestor(table: &dyn ProcessTable, mut pid: Pid) -> Option<Owner> {
    for _ in 0..MAX_DEPTH {
        let stat = table.stat(pid)?;
        let argv = table.cmdline(pid);
        if stat.comm == "codex"
            || argv
                .first()
                .is_some_and(|s| s.rsplit('/').next() == Some("codex"))
        {
            return if embedded(&argv) {
                Some(Owner {
                    pid,
                    start: table.start(pid)?,
                })
            } else {
                None
            };
        }
        if stat.ppid == pid || stat.ppid == 0 {
            return None;
        }
        pid = stat.ppid;
    }
    None
}
pub(super) fn matches(
    info: &ProcessInfo,
    pane: &str,
    owner: &Owner,
    is_embedded: bool,
    table: &dyn ProcessTable,
) -> bool {
    info.pane_id.as_ref().is_some_and(|p| p.as_str() == pane)
        && info
            .foreground_processes
            .as_ref()
            .is_some_and(|procs| procs.iter().any(|p| p.pid == Some(i64::from(owner.pid))))
        && table.start(owner.pid).as_ref() == Some(&owner.start)
        && if is_embedded {
            embedded(&table.cmdline(owner.pid))
        } else {
            codex(&table.cmdline(owner.pid))
        }
}
pub(super) async fn info(herdr: &Herdr, pane: &PaneId) -> Result<ProcessInfo, &'static str> {
    tokio::time::timeout(Duration::from_millis(600), herdr.process_info(pane))
        .await
        .map_err(|_| "pane process lookup timed out")?
        .map_err(|_| "pane process information unavailable")
}
pub(super) async fn check(
    herdr: &Herdr,
    pane: &PaneId,
    owner: &Owner,
    table: &dyn ProcessTable,
) -> Result<(), &'static str> {
    if matches(&info(herdr, pane).await?, pane.as_str(), owner, true, table) {
        Ok(())
    } else {
        Err(
            "cannot prove this hook belongs to the pane's embedded Codex; use codex-bind THREAD_ID --pane PANE_ID",
        )
    }
}
pub(super) fn foreground(
    info: &ProcessInfo,
    pane: &str,
    table: &dyn ProcessTable,
) -> Option<Owner> {
    if info.pane_id.as_ref()?.as_str() != pane {
        return None;
    }
    let mut candidates = info.foreground_processes.as_ref()?.iter().filter_map(|p| {
        let pid = u32::try_from(p.pid?).ok()?;
        if !codex(&table.cmdline(pid)) {
            return None;
        }
        Some(Owner {
            pid,
            start: table.start(pid)?,
        })
    });
    let first = candidates.next()?;
    candidates.next().is_none().then_some(first)
}
