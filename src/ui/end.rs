//! Why a view ended (the panel's or the app's): a clean end, herdr closes the pane with it.

/// Why a view ended.
#[derive(Debug, PartialEq, Eq)]
pub enum End {
    /// The core said the panels go.
    Shutdown,
    /// No core for `no_core::NO_CORE`.
    NoCore,
    /// A signal (the pane closed, or the process was asked to stop).
    Signal,
    /// The terminal's input ended.
    Input,
    /// The core client stopped on its own (its task ended: a panic in it, say).
    ClientGone,
    /// The operator closed it (q).
    Quit,
    /// The terminal went away (the pane closed): reads and writes fail.
    Hangup,
}
