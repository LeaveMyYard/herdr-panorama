//! The record on disk: one atomic file in the writer's generation directory.
use super::state::Saved;
use crate::app_feed::fs::{Home, Known, Result};

pub const FILE: &str = "resume.json";
pub const TEMP: &str = ".resume.tmp";
const LIMIT: usize = 1024 * 1024;

pub struct Store {
    path: String,
    temp: String,
    known: Option<Known>,
}

impl Store {
    pub fn new(generation: &str) -> Self {
        Self {
            path: format!("{generation}/{FILE}"),
            temp: format!("{generation}/{TEMP}"),
            known: None,
        }
    }

    /// The last save. Only the one writer runs here, so a staging file is a crash's leftover.
    pub fn load(&mut self, home: &Home, install_id: &str) -> Result<Option<Saved>> {
        if let Some(temp) = home.read(&self.temp, LIMIT, 0o600)? {
            home.remove(&self.temp, &temp)?;
        }
        self.known = home.read(&self.path, LIMIT, 0o600)?;
        let Some(known) = &self.known else {
            return Ok(None);
        };
        let saved: Saved = serde_json::from_slice(&known.bytes).map_err(|e| e.to_string())?;
        if saved.v != 1 || saved.install_id != install_id {
            return Err("agent resume record belongs to another installation".into());
        }
        Ok(Some(saved))
    }

    /// Durable on return: staged, synced and renamed over the last save.
    pub fn save(&mut self, home: &Home, saved: &Saved) -> Result<()> {
        let bytes = serde_json::to_vec_pretty(saved).map_err(|e| e.to_string())?;
        if bytes.len() > LIMIT {
            return Err("agent resume record is too large".into());
        }
        home.replace(&self.path, &self.temp, &bytes, self.known.as_ref())?;
        self.known = home.read(&self.path, LIMIT, 0o600)?;
        Ok(())
    }

    /// Turned off: nothing stays that could act when it is turned on again.
    pub fn clear(&mut self, home: &Home) -> Result<()> {
        if let Some(known) = home.read(&self.path, LIMIT, 0o600)? {
            home.remove(&self.path, &known)?;
        }
        self.known = None;
        Ok(())
    }
}
