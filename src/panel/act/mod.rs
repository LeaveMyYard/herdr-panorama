//! The herdr side of the panel's effects (Python `PanelApp._switch_worker`, `_prepaint`,
//! `action_goto`'s check, `_goto_remote`, `inbox_link.open_inbox`, `herdr.open_new_agent`):
//! what the actor runs, off its loop, when an input asks for a switch, the inbox or the
//! + New agent popup. Every call goes through the kit's herdr client.
//!
//! - [`switch`]: show the target's tab, the keyboard on its panel (after announcing it, so it
//!   paints its focused look first) or on its agent;
//! - [`goto`]: whether herdr still has a pane; [`remote`]: a pane on another machine;
//! - [`inbox`]: herdr-inbox, deep-linked to a question or a pane's group when it can;
//! - [`new_agent`]: the + New agent popup.

pub mod goto;
pub mod inbox;
pub mod new_agent;
pub mod remote;
pub mod switch;

#[cfg(test)]
mod fake;
