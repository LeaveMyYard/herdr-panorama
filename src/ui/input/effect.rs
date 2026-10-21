//! What a view asks its actor to do after an input: the IO side of a key or a click.

/// A notice in the corner (Python's `notify`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Toast {
    pub title: Option<String>,
    pub text: String,
    pub seconds: u64,
    pub warning: bool,
}

impl Toast {
    /// A notice of `seconds`.
    pub fn new(title: Option<&str>, text: impl Into<String>, seconds: u64) -> Self {
        Self {
            title: title.map(Into::into),
            text: text.into(),
            seconds,
            warning: false,
        }
    }

    /// The same as a warning.
    pub fn warning(mut self) -> Self {
        self.warning = true;
        self
    }
}

/// An effect of an input, for the actor to carry out.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Effect {
    /// The operator looked at these agents: tell the core (`seen`).
    Seen(Vec<String>),
    /// The operator chose this filter here: tell the core (`filter`). If it does not go out,
    /// the actor calls `Panel::filter_not_sent`.
    SendFilter(panorama_core::view::Filter),
    /// Switch to the tab of the orchestrator in `pane` (Python `PanelApp._dock`): onto that
    /// tab's panel when `keep_focus` (after announcing it), else into the agent. The actor
    /// calls `Panel::switched` when it is done.
    Switch { pane: String, keep_focus: bool },
    /// Go to the orchestrator in `pane` if herdr still has it (else say `label` is gone): its
    /// agents are then seen (`Panel::seen_now`), then [`Effect::Switch`].
    Goto {
        pane: String,
        label: String,
        keep_focus: bool,
        seen: Vec<String>,
    },
    /// Focus a pane on another herdr machine and say where it is.
    GotoRemote {
        machine_id: String,
        machine: String,
        pane: String,
        label: String,
    },
    /// Open herdr-inbox: on a question's group, a pane's, or plain.
    OpenInbox {
        question: Option<i64>,
        pane: Option<String>,
    },
    /// The + New agent form in its own herdr popup, `cwd` its default directory.
    NewAgent { cwd: Option<String> },
    /// Show a notice.
    Toast(Toast),
    /// The arrows' switch is due at this instant: call `Panel::arrows_rested` then.
    ArrowsRestAt(std::time::Instant),
    /// The panel closes (herdr closes its pane with it).
    Quit,
    /// The home view's pane full-screen in its tab, and back (z).
    Zoom,
}
