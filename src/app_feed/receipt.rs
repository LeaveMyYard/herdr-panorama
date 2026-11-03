//! The installer receipt is runtime authority only for one exact private artifact/generation.
use super::{
    fs::{Home, Known, Result, file_digest_until},
    json,
};
use serde::{Deserialize, Serialize};
use std::{fs::File, os::unix::fs::MetadataExt};

pub const RECEIPT: &str = "data/panorama/install.json";
pub const BINARY: &str = "data/panorama/bin/panorama";
pub const FEED: &str = "data/panorama/feed.json";
pub const SMALL: usize = 4096;
pub const FEED_LIMIT: usize = 32 * 1024 * 1024;

#[derive(Clone)]
pub struct Receipt {
    pub id: String,
    pub binary_sha: String,
}
impl Receipt {
    pub fn read(home: &Home) -> Result<Self> {
        let bytes = home
            .read(RECEIPT, 1024 * 1024, 0o600)?
            .ok_or("no installer receipt; app-feed cannot start")?;
        let v = json::parse(&bytes.bytes)?;
        json::keys(&v, &["v", "install_id", "files", "dirs", "keys"])?;
        if v["v"].as_i64() != Some(2) {
            return Err(
                "app-feed requires an installer v2 receipt; no legacy runtime authority".into(),
            );
        }
        let id = v["install_id"]
            .as_str()
            .filter(|s| json::hex(s, 32))
            .ok_or("invalid install generation")?
            .to_owned();
        super::inventory::validate(&v)?;
        let files = v["files"].as_object().ok_or("invalid receipt inventory")?;
        if !v["dirs"].is_array() || !(v["keys"].is_null() || v["keys"].is_object()) {
            return Err("invalid receipt metadata".into());
        }
        let binary = files
            .get(BINARY)
            .ok_or("receipt has no private panorama artifact")?;
        json::keys(binary, &["sha256", "mode"])?;
        let sha = binary["sha256"]
            .as_str()
            .filter(|s| json::hex(s, 64))
            .ok_or("invalid binary fingerprint")?;
        if binary["mode"].as_i64() != Some(0o755) {
            return Err("private binary receipt mode must be 0755".into());
        }
        Ok(Self {
            id,
            binary_sha: sha.into(),
        })
    }
    pub fn require_generation(&self, expected: Option<&str>) -> Result<()> {
        if expected.is_some_and(|id| id != self.id) {
            return Err("installer generation changed; refusing stale client".into());
        }
        Ok(())
    }
    pub fn path(&self, name: &str) -> String {
        format!("data/panorama/runtime/{}/{name}", self.id)
    }
    pub fn generation(&self) -> String {
        format!("data/panorama/runtime/{}", self.id)
    }
    pub fn feed_temp(&self) -> String {
        format!("data/panorama/.feed-{}.tmp", self.id)
    }
    pub fn check_artifact(&self, home: &Home) -> Result<()> {
        self.check_artifact_until(home, None)
    }
    pub fn check_artifact_until(
        &self,
        home: &Home,
        deadline: Option<std::time::Instant>,
    ) -> Result<()> {
        let path = home.path.join(BINARY);
        if std::env::current_exe().map_err(|e| e.to_string())? != path {
            return Err("app-feed must run the exact installer-owned private binary".into());
        }
        let file = home
            .open_file(BINARY, false)?
            .ok_or("private binary is absent")?;
        let before = file.metadata().map_err(|e| e.to_string())?;
        if before.mode() & 0o7777 != 0o755 || file_digest_until(&file, deadline)? != self.binary_sha
        {
            return Err("private binary hash/mode mismatch; preserved".into());
        }
        let after = file.metadata().map_err(|e| e.to_string())?;
        if (
            before.dev(),
            before.ino(),
            before.len(),
            before.mtime(),
            before.mtime_nsec(),
        ) != (
            after.dev(),
            after.ino(),
            after.len(),
            after.mtime(),
            after.mtime_nsec(),
        ) {
            return Err("private artifact changed during validation".into());
        }
        Ok(())
    }
    pub fn lock_file(&self, home: &Home) -> Result<File> {
        let path = self.path("writer.lock");
        let file = home
            .open_file(&path, true)?
            .ok_or("installer lifetime lock is absent")?;
        let m = file.metadata().map_err(|e| e.to_string())?;
        if m.mode() & 0o7777 != 0o600 || m.len() != 0 {
            return Err("invalid installer lifetime lock".into());
        }
        Ok(file)
    }
    pub fn check_names(&self, home: &Home) -> Result<()> {
        self.names(home, false)
    }
    pub fn check_busy_names(&self, home: &Home) -> Result<()> {
        self.names(home, true)
    }
    fn names(&self, home: &Home, busy: bool) -> Result<()> {
        let feed_temp = format!(".feed-{}.tmp", self.id);
        for name in home.names("data/panorama")? {
            if ![
                "install.json",
                "bin",
                "runtime",
                "feed.json",
                feed_temp.as_str(),
            ]
            .contains(&name.as_str())
            {
                return Err(format!("unknown runtime parent entry {name}; preserved"));
            }
        }
        if home.names("data/panorama/runtime")? != [self.id.clone()] {
            return Err("unknown runtime generation; preserved".into());
        }
        for name in home.names(&self.generation())? {
            if !(["control.json", "writer.lock", "ownership.json"].contains(&name.as_str())
                || busy && name == ".ownership.tmp")
            {
                return Err(format!("ambiguous runtime entry {name}; preserved"));
            }
        }
        Ok(())
    }
    pub fn control(&self, home: &Home) -> Result<(Control, Known)> {
        let known = home
            .read(&self.path("control.json"), SMALL, 0o600)?
            .ok_or("installer control is absent")?;
        let v = json::parse(&known.bytes)?;
        json::keys(&v, &["v", "install_id", "disabled"])?;
        let c: Control = serde_json::from_value(v).map_err(|e| e.to_string())?;
        if c.v != 1 || c.install_id != self.id {
            return Err("foreign runtime control".into());
        }
        Ok((c, known))
    }
    pub fn enabled(&self, home: &Home) -> Result<()> {
        if self.control(home)?.0.disabled {
            return Err("installation runtime is disabled".into());
        }
        Ok(())
    }
    pub fn active(&self, home: &Home) -> Result<()> {
        let current = Self::read(home)?;
        if current.id != self.id || current.binary_sha != self.binary_sha {
            return Err("installer generation changed".into());
        }
        self.enabled(home)
    }
    pub fn disable(&self, home: &Home) -> Result<()> {
        let (mut control, known) = self.control(home)?;
        control.disabled = true;
        home.replace(
            &self.path("control.json"),
            &self.path(".control.tmp"),
            &serde_json::to_vec(&control).map_err(|e| e.to_string())?,
            Some(&known),
        )
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Control {
    pub v: u8,
    pub install_id: String,
    pub disabled: bool,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ownership {
    pub v: u8,
    pub install_id: String,
    pub binary_sha256: String,
    pub writer_pid: u32,
    pub writer_start: String,
    pub writer_pidns: Option<String>,
    pub feed_sha256: Option<String>,
    pub pending_feed_sha256: Option<String>,
}
impl Ownership {
    pub fn read(home: &Home, r: &Receipt) -> Result<Option<(Self, Known)>> {
        let Some(known) = home.read(&r.path("ownership.json"), SMALL, 0o600)? else {
            return Ok(None);
        };
        let v = json::parse(&known.bytes)?;
        json::keys(
            &v,
            &[
                "v",
                "install_id",
                "binary_sha256",
                "writer_pid",
                "writer_start",
                "writer_pidns",
                "feed_sha256",
                "pending_feed_sha256",
            ],
        )?;
        let o: Self = serde_json::from_value(v).map_err(|e| e.to_string())?;
        if o.v != 1
            || o.install_id != r.id
            || o.binary_sha256 != r.binary_sha
            || o.writer_pid <= 1
            || !json::token(&o.writer_start)
            || o.writer_pidns.as_deref().is_some_and(|s| !json::token(s))
            || [o.feed_sha256.as_deref(), o.pending_feed_sha256.as_deref()]
                .into_iter()
                .flatten()
                .any(|s| !json::hex(s, 64))
        {
            return Err("runtime ownership provenance mismatch; preserved".into());
        }
        Ok(Some((o, known)))
    }
    pub fn write(&self, home: &Home, r: &Receipt, expected: Option<&Known>) -> Result<()> {
        home.replace(
            &r.path("ownership.json"),
            &r.path(".ownership.tmp"),
            &serde_json::to_vec(self).map_err(|e| e.to_string())?,
            expected,
        )
    }
    pub fn live(&self, home: &Home) -> bool {
        use herdr_agent_kit::agent_procs::{ProcessTable, SystemProcesses};
        let table = SystemProcesses::new();
        if !crate::host::records::namespace_matches(self.writer_pidns.as_deref())
            || table.start(self.writer_pid).as_deref() != Some(&self.writer_start)
        {
            return false;
        }
        let argv = table.cmdline(self.writer_pid);
        argv == [
            home.path.join(BINARY).to_string_lossy().into_owned(),
            "app-feed".into(),
            "--watch".into(),
            "--app-home".into(),
            home.path.to_string_lossy().into_owned(),
            "--install-id".into(),
            self.install_id.clone(),
        ]
    }
}
