//! What the home view's herdr side reports back to the actor (Python's `_to_ui` calls).

use crate::ui::input::Toast;
use std::collections::BTreeMap;

#[derive(Debug)]
pub enum Done {
    /// A notice to show.
    Toast(Toast),
    /// A popup's job is done (going to a pane, a started agent): it closes.
    Exit,
    /// Each tab's panel, after a panels pass (published when it changed).
    Panels(BTreeMap<String, String>),
    /// herdr shows this tab.
    TabShown(String),
    /// herdr says this pane got the focus.
    HerdrFocus(String),
    /// An announced focus was not confirmed in time: herdr's real focus (Python `_focus_read`).
    FocusRead {
        announced: String,
        real: Option<String>,
    },
    /// The home pane has a new id (moved back to its own tab).
    MyPane(String),
    /// The + New agent form, `cwd` its directory (PR 9b).
    NewAgentForm(String),
    /// The popup closes, then herdr-inbox opens (a popup is modal: Python's `after_exit`).
    ExitThenInbox {
        question: Option<i64>,
        pane: Option<String>,
    },
}
