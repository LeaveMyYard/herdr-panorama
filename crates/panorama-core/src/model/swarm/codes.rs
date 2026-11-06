//! The small code words of the swarm tree: what herdr says of a root's pane. Each reads from
//! its wire string; a word this version does not know is kept as `Other`, so nothing sent is
//! lost.

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
            /// A word this version does not know, as it was sent. Never a known word: build
            /// values with `from_code` (or serde), not this variant by hand.
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
    /// What herdr reports for a root's pane (`unknown` until it says).
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

#[cfg(test)]
mod tests;
