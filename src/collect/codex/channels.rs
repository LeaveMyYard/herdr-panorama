//! Bounded explicit reports, scoped by exact source identity. Partial or stale
//! observations never authorize reading descendant event files.

use super::tree;
use crate::codex::Observation;
use crate::collect::events::{Channel, Declared};
use crate::collect::paths::events_path;
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

const MAX_CHANNELS: usize = 8;
const MAX_FILE: u64 = 128 * 1024;
const MAX_BYTES: u64 = 512 * 1024;
const MAX_DECLARATIONS: usize = 64;
const MAX_AGE: f64 = 15.0;

#[derive(Clone, Debug)]
pub(super) struct Reports {
    pub source: String,
    pub declarations: Vec<Declared>,
}

struct Cached {
    modified: Option<SystemTime>,
    len: u64,
    declarations: Vec<Declared>,
}

pub(super) struct Channels {
    dir: PathBuf,
    cache: HashMap<String, Cached>,
    remaining_bytes: u64,
    remaining_channels: usize,
}

impl Channels {
    pub(super) fn new(dir: PathBuf) -> Self {
        Self {
            dir,
            cache: HashMap::new(),
            remaining_bytes: MAX_BYTES,
            remaining_channels: 32,
        }
    }

    pub(super) fn begin_collect(&mut self) {
        self.remaining_bytes = MAX_BYTES;
        self.remaining_channels = 32;
    }

    pub(super) fn read(
        &mut self,
        root: &str,
        observation: Option<&Observation>,
        now: f64,
    ) -> Vec<Reports> {
        let mut sources = Vec::new();
        if valid_id(root) {
            sources.push(root.to_owned());
        }
        if let Some(observation) = observation.filter(|value| {
            valid_id(root)
                && value.root.thread_id == root
                && value.root.parent_thread_id.is_none()
                && !value.partial
                && (0.0..=MAX_AGE).contains(&(now - value.observed_at))
        }) {
            sources.extend(
                tree::descendants(observation)
                    .into_iter()
                    .map(|(_, thread)| thread.thread_id.clone())
                    .filter(|id| valid_id(id))
                    .take(MAX_CHANNELS.saturating_sub(sources.len())),
            );
        }
        sources.truncate(self.remaining_channels);
        self.remaining_channels -= sources.len();
        let mut budget = self.remaining_bytes;
        let reports = sources
            .into_iter()
            .map(|source| {
                let path = events_path(&self.dir, &source);
                let declarations = self.load(&source, &path, &mut budget);
                Reports {
                    source,
                    declarations,
                }
            })
            .collect();
        self.remaining_bytes = budget;
        reports
    }

    fn load(&mut self, id: &str, path: &Path, budget: &mut u64) -> Vec<Declared> {
        let metadata = fs::symlink_metadata(path)
            .ok()
            .filter(|meta| meta.is_file() && meta.len() <= MAX_FILE);
        let Some(metadata) = metadata else {
            self.cache.remove(id);
            return Vec::new();
        };
        if let Some(cached) = self.cache.get(id).filter(|cached| {
            cached.modified == metadata.modified().ok() && cached.len == metadata.len()
        }) {
            return cached.declarations.clone();
        }
        if metadata.len() > *budget {
            return Vec::new();
        }
        *budget -= metadata.len();
        let declarations = read(path, metadata.len());
        if self.cache.len() >= 128 && !self.cache.contains_key(id) {
            self.cache.clear();
        }
        self.cache.insert(
            id.into(),
            Cached {
                modified: metadata.modified().ok(),
                len: metadata.len(),
                declarations: declarations.clone(),
            },
        );
        declarations
    }
}

fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

fn read(path: &Path, len: u64) -> Vec<Declared> {
    let mut text = String::new();
    if File::open(path)
        .and_then(|file| file.take(len + 1).read_to_string(&mut text))
        .is_err()
        || text.len() as u64 != len
    {
        return Vec::new();
    }
    let mut channel = Channel::new(path.to_owned());
    for line in text.lines() {
        if let Ok(serde_json::Value::Object(value)) = serde_json::from_str(line) {
            if channel.threads.len() >= MAX_DECLARATIONS
                && !channel.threads.iter().any(|declaration| {
                    Some(declaration.thread.as_str())
                        == value.get("thread").and_then(serde_json::Value::as_str)
                })
            {
                continue;
            }
            channel.apply(&value);
        }
    }
    for declaration in &mut channel.threads {
        declaration.phases.truncate(crate::report::MAX_ITEMS);
        declaration.agents.truncate(crate::report::MAX_ITEMS);
    }
    channel.threads
}

#[cfg(test)]
mod tests;
