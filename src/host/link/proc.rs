//! Bounded reads of one process's own records under a procfs root (injected for tests).
use std::{
    io::Read,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
};

/// The exec-environment values the gate uses; nothing else is kept.
#[derive(Default, Debug, PartialEq, Eq)]
pub struct Environment {
    pub terminal: Option<String>,
    pub claude_config: Option<String>,
    pub home: Option<String>,
}

impl Environment {
    /// The process's effective `${CLAUDE_CONFIG_DIR:-$HOME/.claude}`. Only an absolute path: a
    /// relative one (or an empty value) means something else from Panorama's cwd.
    pub fn claude_dir(&self) -> Option<PathBuf> {
        let dir = match (&self.claude_config, &self.home) {
            (Some(dir), _) => PathBuf::from(dir),
            (None, Some(home)) => Path::new(home).join(".claude"),
            (None, None) => return None,
        };
        dir.is_absolute().then_some(dir)
    }
}

/// A larger exec environment is refused, never partially trusted.
const MAX_ENVIRON: u64 = 256 * 1024;

pub struct Proc {
    root: PathBuf,
    uid: u32,
}

impl Proc {
    pub fn new(root: impl Into<PathBuf>, uid: u32) -> Self {
        Self {
            root: root.into(),
            uid,
        }
    }

    fn dir(&self, pid: u32) -> PathBuf {
        self.root.join(pid.to_string())
    }

    /// The process directory belongs to Panorama's own user.
    pub fn owned(&self, pid: u32) -> bool {
        std::fs::metadata(self.dir(pid)).is_ok_and(|m| m.uid() == self.uid)
    }

    /// `HC_TERMINAL_ID`, `CLAUDE_CONFIG_DIR` and `HOME` from the exec environment. Only these
    /// keys are kept; the rest of the buffer is dropped here and never logged or returned. A key
    /// that appears twice, or an environment over the bound, is refused.
    pub fn environment(&self, pid: u32) -> Option<Environment> {
        let file = std::fs::File::open(self.dir(pid).join("environ")).ok()?;
        let mut bytes = Vec::new();
        file.take(MAX_ENVIRON + 1).read_to_end(&mut bytes).ok()?;
        if bytes.len() as u64 > MAX_ENVIRON {
            return None;
        }
        let mut env = Environment::default();
        for entry in bytes.split(|b| *b == 0) {
            for (key, slot) in [
                (&b"HC_TERMINAL_ID="[..], &mut env.terminal),
                (&b"CLAUDE_CONFIG_DIR="[..], &mut env.claude_config),
                (&b"HOME="[..], &mut env.home),
            ] {
                if let Some(value) = entry.strip_prefix(key) {
                    if slot.is_some() {
                        return None;
                    }
                    *slot = Some(String::from_utf8(value.to_vec()).ok()?);
                }
            }
        }
        Some(env)
    }
}
