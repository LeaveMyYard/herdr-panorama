//! The small code words of the swarm tree: what herdr says of a pane, a thread's status and
//! source, a phase's state. Each reads from its wire string; a word this version does not
//! know is kept as `Other`, so nothing sent is lost.

use serde::{Deserialize, Serialize};

macro_rules! code_enum {
    (
        $(#[$doc:meta])*
        $name:ident $(, default $default:ident)? { $($(#[$vdoc:meta])* $variant:ident = $code:literal,)+ }
    ) => {
        $(#[$doc])*
        #[derive(Clone, Debug, PartialEq, Eq, Hash, Deserialize, Serialize)]
        #[serde(from = "String", into = "String")]
        pub enum $name {
            $($(#[$vdoc])* $variant,)+
            /// A word this version does not know, as it was sent.
            Other(String),
        }

        impl $name {
            /// The value for a wire word.
            pub fn from_code(code: &str) -> Self {
                match code {
                    $($code => Self::$variant,)+
                    other => Self::Other(other.to_owned()),
                }
            }

            /// The wire word.
            pub fn code(&self) -> &str {
                match self {
                    $(Self::$variant => $code,)+
                    Self::Other(code) => code,
                }
            }
        }

        impl From<String> for $name {
            fn from(code: String) -> Self {
                Self::from_code(&code)
            }
        }

        impl From<$name> for String {
            fn from(v: $name) -> Self {
                v.code().to_owned()
            }
        }

        $(
            impl Default for $name {
                fn default() -> Self {
                    Self::$default
                }
            }
        )?
    };
}

code_enum! {
    /// What herdr reports for an orchestrator's pane (`unknown` until it says).
    HerdrStatus, default Unknown {
        /// The agent is working.
        Working = "working",
        /// It waits on a prompt.
        Blocked = "blocked",
        /// Idle.
        Idle = "idle",
        /// Finished.
        Done = "done",
        /// herdr did not say.
        Unknown = "unknown",
    }
}

code_enum! {
    /// A thread's status.
    ThreadStatus, default Active {
        /// Going.
        Active = "active",
        /// Finished.
        Done = "done",
        /// Failed: it stays shown, it needs the operator.
        Failed = "failed",
        /// Cancelled.
        Cancelled = "cancelled",
    }
}

code_enum! {
    /// Where a thread was found.
    ThreadSource {
        /// Reported through the explicit channel.
        Explicit = "explicit",
        /// A workflow run.
        Workflow = "workflow",
        /// A reference (a PR, an issue) in the agents' descriptions.
        Ref = "ref",
        /// One turn of the session.
        Turn = "turn",
        /// The session's own work.
        Session = "session",
    }
}

code_enum! {
    /// A phase's state.
    PhaseState {
        /// Done.
        Done = "done",
        /// The current one.
        Now = "now",
        /// Still to come.
        Next = "next",
        /// Failed.
        Failed = "failed",
        /// Skipped.
        Skipped = "skipped",
    }
}

#[cfg(test)]
mod tests;
