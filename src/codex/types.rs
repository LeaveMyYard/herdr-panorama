use herdr_agent_kit::plugin_env::PluginEnv;
use std::path::PathBuf;
use std::time::Duration;

/// Process location and bounded refresh policy. Tests can supply a fake executable.
#[derive(Clone, Debug)]
pub struct Options {
    pub executable: PathBuf,
    pub codex_home: PathBuf,
    /// Whole refresh deadline, including child reads and process initialization.
    pub timeout: Duration,
    pub refresh_interval: Duration,
}
impl Options {
    pub fn from_env(env: &PluginEnv) -> Option<Self> {
        Some(Self {
            executable: "codex".into(),
            codex_home: env
                .path_var("CODEX_HOME")
                .map(PathBuf::from)
                .or_else(|| env.home().map(|p| p.join(".codex")))?,
            timeout: Duration::from_secs(4),
            refresh_interval: Duration::from_secs(5),
        })
    }
}

/// A persisted turn outcome. Even InProgress may be stale; it is not runtime telemetry.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum TurnStatus {
    Completed,
    Failed,
    Interrupted,
    InProgress,
    #[default]
    Unknown,
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Tool {
    pub name: String,
    pub turn_id: String,
    pub started_at: Option<f64>,
    pub completed_at: Option<f64>,
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Turn {
    pub id: String,
    pub status: TurnStatus,
    pub started_at: Option<f64>,
    pub completed_at: Option<f64>,
    pub prompt: String,
    pub summary: String,
    /// Count in the bounded item window, not an all-time total.
    pub tool_count: usize,
    pub latest_tool: Option<Tool>,
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Thread {
    pub thread_id: String,
    pub session_tree_id: String,
    /// None means an explicit null parent; missing/malformed ancestry is refused.
    pub parent_thread_id: Option<String>,
    pub forked_from_id: Option<String>,
    pub name: Option<String>,
    /// The spawner-given task name: the last segment of `source.subAgent.thread_spawn.agent_path`.
    pub task_name: Option<String>,
    pub nickname: Option<String>,
    /// Codex's `preview`: usually the thread's first user message, whatever the read window.
    pub preview: Option<String>,
    pub role: Option<String>,
    pub model: Option<String>,
    pub created_at: f64,
    pub updated_at: f64,
    /// Newest first. Completion belongs only to this turn, never the whole thread.
    pub turns: Vec<Turn>,
    pub latest_tool: Option<Tool>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Observation {
    pub root: Thread,
    /// Explicitly linked descendants, with parent IDs retained for nesting.
    pub children: Vec<Thread>,
    /// Page/thread limit, unsupported history API or child read failure.
    pub partial: bool,
    /// Local time when this bounded read completed; not a runtime freshness guarantee.
    pub observed_at: f64,
}
