//! `panorama open [--popup] [--split]` and `panorama home [--startup]` (Python
//! `cli.cmd_open`, `cli.cmd_home`): the plugin's `open`/`open-popup`/`open-split` actions and
//! herdr's startup hook. SAFETY: they open panorama's pane and focus its tab.
//!
//! The open key: in per-tab mode the core decides first (this tab's panel, made if missing,
//! or the home tab); without a core, or when it could do neither, the home tab (opened if
//! missing); when the home tab is off or cannot be shown, the view as a popup. `--popup` and
//! `--split` go straight to the view.

mod real;
mod request;

pub use real::RealOpen;
pub use request::{OPEN_WAIT, request_open};

use crate::home::config::{home_enabled, per_tab};
use serde_json::{Map, Value};
use std::io::Write;

/// The herdr side of the commands.
pub trait OpenOps {
    /// The core's answer to the open key (`panel`, `home`, or why it did neither); `None`
    /// when there is no core of ours to ask.
    fn request_open(&self) -> impl Future<Output = Option<String>> + Send;
    /// The home tab, opened if missing; focused or not.
    fn ensure_home(&self, focus: bool) -> impl Future<Output = Result<(), String>> + Send;
    /// The plugin's `panorama` pane: its own placement (the popup), or a right split of the
    /// pane the action was invoked from.
    fn open_view(&self, split: bool) -> impl Future<Output = Result<(), String>> + Send;
}

/// `open`'s flags.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OpenArgs {
    pub split: bool,
    pub popup: bool,
}

/// The arguments after the subcommand; `Err` names one it does not take (argparse's exit 2).
pub fn parse_flags(args: &[String], known: &[&'static str]) -> Result<Vec<&'static str>, String> {
    args.iter()
        .map(|a| {
            known
                .iter()
                .find(|k| **k == a)
                .copied()
                .ok_or_else(|| format!("unrecognized arguments: {a}"))
        })
        .collect()
}

/// The home tab (opened if missing), focused or not: what [`OpenOps::ensure_home`] runs.
pub async fn ensure_home(
    home: &impl crate::home::HomeOps,
    places: &crate::home::Places,
    focus: bool,
) -> Result<(), String> {
    crate::home::ensure(home, places, focus).await.map(drop)
}

/// `panorama open`: the exit status (1 when not even the view could be opened).
pub async fn open(
    ops: &impl OpenOps,
    cfg: &Map<String, Value>,
    a: OpenArgs,
    err: &mut impl Write,
) -> i32 {
    let plain = !a.split && !a.popup;
    if plain && home_enabled(cfg) && per_tab(cfg) {
        match ops.request_open().await.as_deref() {
            Some("panel" | "home") => return 0,
            Some(did) => {
                let _ = writeln!(err, "panorama: open: {did}; showing the home tab");
            }
            None => {} // no core of ours: the home tab
        }
    }
    if plain && home_enabled(cfg) {
        match ops.ensure_home(true).await {
            Ok(()) => return 0,
            Err(e) => {
                let _ = writeln!(err, "panorama: home tab: {e}; opening the popup");
            }
        }
    }
    match ops.open_view(a.split).await {
        Ok(()) => 0,
        Err(e) => {
            let _ = writeln!(err, "panorama: could not open the view: {e}");
            1
        }
    }
}

/// `panorama home`: the home tab, focused (`--startup`: once, without the focus); nothing when
/// the home tab is off.
pub async fn home(
    ops: &impl OpenOps,
    cfg: &Map<String, Value>,
    startup: bool,
    err: &mut impl Write,
) -> i32 {
    if !home_enabled(cfg) {
        return 0;
    }
    match ops.ensure_home(!startup).await {
        Ok(()) => 0,
        Err(e) => {
            let _ = writeln!(err, "panorama: home tab: {e}");
            1
        }
    }
}

/// `panorama open|home <flags>` from `main`: the exit status.
pub fn main(cmd: &str, args: &[String]) -> i32 {
    let known: &[&'static str] = if cmd == "open" {
        &["--split", "--popup"]
    } else {
        &["--startup"]
    };
    let flags = match parse_flags(args, known) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("panorama {cmd}: error: {e}");
            return 2;
        }
    };
    let env = herdr_agent_kit::plugin_env::PluginEnv::from_env();
    let Ok(rt) = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    else {
        return 1;
    };
    rt.block_on(async {
        let ops = RealOpen::from_env(&env);
        let cfg = crate::settings::Settings::from_env(&env).panorama_table();
        let mut err = std::io::stderr();
        if cmd == "open" {
            let a = OpenArgs {
                split: flags.contains(&"--split"),
                popup: flags.contains(&"--popup"),
            };
            open(&ops, &cfg, a, &mut err).await
        } else {
            home(&ops, &cfg, flags.contains(&"--startup"), &mut err).await
        }
    })
}

#[cfg(test)]
mod tests;
