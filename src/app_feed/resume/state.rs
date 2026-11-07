//! What agent resume keeps across a core restart (docs/protocol/agent-resume.md).
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;

/// How the recorded process was started. Only launches on the allow-list may be typed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Launch {
    /// Plain and interactive: the resume command starts the same kind of session.
    Plain,
    /// Anything else, such as `claude --remote-control` (herdr#5130) or `-p`. Unknown is here too.
    Other,
}

/// Which Claude config dir a session uses, proved by where its process's record is.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Config {
    /// `~/.claude`: what `claude` uses with nothing set.
    Default,
    Named(String),
    Unproved,
}

/// One terminal's agent as last seen, written before any loss.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Record {
    pub address: String,
    pub harness: String,
    pub session: String,
    pub cwd: String,
    /// The terminal's own program and its foreground agent: both end with a real loss.
    pub root_pid: u32,
    pub agent_pid: u32,
    /// With the address, which terminal this is.
    pub created_ms: u64,
    pub launch: Launch,
    /// Started with options the resume command doesn't repeat, such as `--model`.
    pub dropped: bool,
    /// Codex with `--no-daemon`: hc needs it, or Codex's commands run outside the terminal.
    pub no_daemon: bool,
    /// Where this session's account and settings live.
    pub config: Config,
    /// The link held at the last look. False means the id may be out of date.
    pub proved: bool,
    /// When the agent was first missing, while the core carried on.
    pub ending_ms: Option<u64>,
}

impl Record {
    fn note(&self) -> Option<&'static str> {
        if self.harness == "claude" && self.config == Config::Unproved {
            Some("its config dir isn't proved, so this resumes under the shell's config")
        } else if self.dropped {
            Some("started with options that aren't repeated")
        } else {
            None
        }
    }

    /// What to type, only for an id that is exactly a UUID: this text reaches a shell. `env`
    /// works the same in bash, zsh and fish. A dir that can't be quoted plainly gives none.
    pub fn command(&self) -> Option<String> {
        let id = herdr_agent_kit::agent_session::SessionId::parse(&self.session)?;
        let id = id.as_str();
        match (self.harness.as_str(), self.no_daemon, &self.config) {
            ("claude", _, Config::Named(dir)) => {
                let plain =
                    dir.starts_with('/') && !dir.chars().any(|c| c == '\'' || c.is_control());
                plain.then(|| format!("env CLAUDE_CONFIG_DIR='{dir}' claude --resume {id}"))
            }
            // A startup file may export another dir, so `~/.claude` is named by unsetting.
            ("claude", _, Config::Default) => {
                Some(format!("env -u CLAUDE_CONFIG_DIR claude --resume {id}"))
            }
            ("claude", _, _) => Some(format!("claude --resume {id}")),
            ("codex", true, _) => Some(format!("codex --no-daemon resume {id}")),
            ("codex", false, _) => Some(format!("codex resume {id}")),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Stage {
    /// No shell to judge yet.
    Waiting,
    /// Written before the send: found again after a restart, it is never sent twice.
    Sending,
    /// The one attempt is spent; only reading from here on.
    Sent,
    Resumed,
    Skipped,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    pub record: Record,
    pub stage: Stage,
    pub reason: Option<String>,
    pub sent_ms: Option<u64>,
    /// Something ran in front of the shell after the send.
    pub started: bool,
    /// Why this loss may only be resumed by a click. The terminal checks still run.
    pub hold: Option<String>,
    /// Panorama saw the terminal with no shell or a blank screen, so `first` is the shell's
    /// first drawn screen and nobody can have typed before it.
    pub witnessed: bool,
    /// A digest of the first screen seen. The line is empty only if the screen still equals it.
    pub first: Option<String>,
    /// The prompt line as last seen, digits blanked. The cursor is only known to be on a line
    /// that starts with this.
    pub prompt: Option<String>,
    /// The screen moved after first sight: never typed by itself, only watched until it stops.
    pub moved: bool,
    /// The last click acted on.
    pub click: Option<u64>,
    /// Nothing a person does can make this terminal typable: no button.
    pub dead: bool,
    /// A reboot was proved, so a repeated pid means nothing.
    pub rebooted: bool,
    /// What the first click found, shown on the row until the second.
    pub look: Option<Look>,
}

/// The terminal as a person is shown it. The send must find the same screen.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Look {
    pub token: String,
    pub shape: String,
    /// The last few non-blank lines, and more of the screen for the tooltip.
    pub lines: Vec<String>,
    pub text: String,
    pub at_ms: u64,
}

impl Entry {
    pub fn new(record: Record) -> Self {
        Self {
            record,
            stage: Stage::Waiting,
            reason: None,
            sent_ms: None,
            started: false,
            hold: None,
            witnessed: false,
            first: None,
            prompt: None,
            moved: false,
            click: None,
            dead: false,
            rebooted: false,
            look: None,
        }
    }
    pub fn end(&mut self, stage: Stage, reason: impl Into<String>) {
        self.stage = stage;
        self.reason = Some(reason.into());
    }
    pub fn open(&self) -> bool {
        matches!(self.stage, Stage::Waiting | Stage::Sending | Stage::Sent)
    }
    /// The button can still lead somewhere for this one.
    pub fn clickable(&self) -> bool {
        matches!(self.stage, Stage::Skipped | Stage::Failed)
            && !self.dead
            && self.record.command().is_some()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Loss {
    pub detected_ms: u64,
    pub entries: BTreeMap<String, Entry>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Saved {
    pub v: u8,
    pub install_id: String,
    /// The core's permanent id, and its holders at the last look.
    pub core: Option<String>,
    pub holders: Vec<String>,
    /// The kernel's id for this boot at the last look.
    pub boot: Option<String>,
    /// When a writer last looked, within a minute. A record can be this old.
    pub seen_ms: u64,
    /// The writer left on purpose: nobody watched from then on.
    pub closed: bool,
    pub records: BTreeMap<String, Record>,
    pub loss: Option<Loss>,
}

impl Saved {
    pub fn new(install_id: &str) -> Self {
        Self {
            v: 1,
            install_id: install_id.into(),
            core: None,
            holders: Vec::new(),
            boot: None,
            seen_ms: 0,
            closed: false,
            records: BTreeMap::new(),
            loss: None,
        }
    }

    /// The feed's `resume` field (docs/protocol/app-feed-v2.md).
    pub fn report(&self, error: Option<&str>) -> Value {
        let loss = self.loss.as_ref().map(|loss| {
            let entries: Vec<Value> = loss
                .entries
                .values()
                .map(|e| {
                    json!({
                        "terminal": e.record.address,
                        "harness": e.record.harness,
                        "session_id": e.record.session,
                        "cwd": e.record.cwd,
                        "state": e.stage,
                        "reason": e.reason,
                        "command": e.record.command(),
                        "note": e.record.note(),
                        "click": e.clickable(),
                        "look": e.look.as_ref().map(|l| json!({"lines":l.lines,"text":l.text,"token":l.token})),
                    })
                })
                .collect();
            json!({"id": loss.detected_ms, "detected_at": loss.detected_ms as f64 / 1000.0, "entries": entries})
        });
        json!({"tracked": self.records.len(), "loss": loss, "error": error})
    }
}
