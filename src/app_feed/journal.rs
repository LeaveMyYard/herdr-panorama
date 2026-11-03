//! A two-digest journal proves either side of an interrupted atomic feed replacement.
use super::{
    fs::{Home, Known, Result, digest},
    receipt::{FEED, FEED_LIMIT, Ownership, Receipt},
};
use herdr_agent_kit::agent_procs::{own_pidns, start_token};

struct Recovery {
    old: Option<(Ownership, Known)>,
    current: Option<String>,
    temp: Option<Known>,
}

pub struct Journal {
    pub owner: Ownership,
    known: Known,
}
impl Journal {
    fn inspect(home: &Home, r: &Receipt) -> Result<Recovery> {
        r.check_names(home)?;
        r.active(home)?;
        let old = Ownership::read(home, r)?;
        let feed = home.read(FEED, FEED_LIMIT, 0o600)?;
        let temp = home.read(&r.feed_temp(), FEED_LIMIT, 0o600)?;
        let current = feed.as_ref().map(|f| digest(&f.bytes));
        if let Some((owner, _)) = &old {
            if let Some(hash) = &current {
                if owner.feed_sha256.as_ref() != Some(hash)
                    && owner.pending_feed_sha256.as_ref() != Some(hash)
                {
                    return Err("feed fingerprint is not owned; preserved".into());
                }
            } else if owner.feed_sha256.is_some() {
                return Err("previously owned feed is missing; runtime is ambiguous".into());
            }
            if let Some(temp) = &temp
                && owner.pending_feed_sha256.as_deref() != Some(digest(&temp.bytes).as_str())
            {
                return Err("unproved feed staging file; preserved".into());
            }
        } else if feed.is_some() || temp.is_some() {
            return Err("existing feed has no generation ownership; preserved".into());
        }
        Ok(Recovery { old, current, temp })
    }
    pub fn validate(home: &Home, r: &Receipt) -> Result<()> {
        Self::inspect(home, r).map(|_| ())
    }
    pub fn open(home: &Home, r: &Receipt) -> Result<Self> {
        let Recovery { old, current, temp } = Self::inspect(home, r)?;
        if let Some(temp) = temp {
            home.remove(&r.feed_temp(), &temp)?;
        }
        let pid = std::process::id();
        let owner = Ownership {
            v: 1,
            install_id: r.id.clone(),
            binary_sha256: r.binary_sha.clone(),
            writer_pid: pid,
            writer_start: start_token(pid).ok_or("cannot prove writer process start identity")?,
            writer_pidns: own_pidns(),
            feed_sha256: current,
            pending_feed_sha256: None,
        };
        owner.write(home, r, old.as_ref().map(|(_, known)| known))?;
        let known = Ownership::read(home, r)?
            .ok_or("writer registration vanished")?
            .1;
        Ok(Self { owner, known })
    }
    fn record(&mut self, home: &Home, r: &Receipt) -> Result<()> {
        self.owner.write(home, r, Some(&self.known))?;
        self.known = Ownership::read(home, r)?
            .ok_or("writer registration vanished")?
            .1;
        Ok(())
    }
    pub fn publish(&mut self, home: &Home, r: &Receipt, bytes: &[u8]) -> Result<()> {
        if bytes.len() > FEED_LIMIT {
            return Err("feed exceeds 32 MiB publication limit".into());
        }
        r.check_names(home)?;
        r.active(home)?;
        let value: serde_json::Value =
            serde_json::from_slice(bytes).map_err(|_| "feed is not complete JSON")?;
        if value["v"].as_i64() != Some(1) {
            return Err("feed has unsupported version".into());
        }
        let old = home.read(FEED, FEED_LIMIT, 0o600)?;
        if old.as_ref().map(|f| digest(&f.bytes)) != self.owner.feed_sha256 {
            return Err("feed changed outside its writer; preserved".into());
        }
        let hash = digest(bytes);
        self.owner.pending_feed_sha256 = Some(hash.clone());
        self.record(home, r)?;
        r.active(home)?;
        home.replace_checked(FEED, &r.feed_temp(), bytes, old.as_ref(), || r.active(home))?;
        self.owner.feed_sha256 = Some(hash);
        self.owner.pending_feed_sha256 = None;
        self.record(home, r)
    }
}
