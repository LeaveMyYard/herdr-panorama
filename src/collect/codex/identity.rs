//! Local association provenance lookup. Cache only within one collection, so a
//! later snapshot can never reuse another foreground process's cached identity.

use herdr_agent_kit::{herdr_api::blocking::Herdr, plugin_env::PluginEnv};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::time::{Duration, Instant};

const BUDGET: Duration = Duration::from_millis(100);
const REQUEST: Duration = Duration::from_millis(50);
const MAX_PANES: usize = 32;

pub(in crate::collect) struct LocalIdentity {
    env: PluginEnv,
    herdr: Herdr,
    cache: HashMap<String, Option<Value>>,
    deadline: Instant,
}

impl LocalIdentity {
    pub(in crate::collect) fn new(env: PluginEnv, herdr: Herdr) -> Self {
        Self {
            env,
            herdr,
            cache: HashMap::new(),
            deadline: Instant::now() + BUDGET,
        }
    }

    pub(in crate::collect) fn begin_collect(&mut self) {
        self.cache.clear();
        self.deadline = Instant::now() + BUDGET;
    }

    pub(in crate::collect) fn resolve(
        &mut self,
        pane: &str,
        native: Option<&str>,
    ) -> Option<String> {
        let env = self.env.clone();
        crate::codex::identity::resolve(&env, pane, native, || self.process(pane))
    }

    fn process(&mut self, pane: &str) -> Option<Value> {
        if let Some(value) = self.cache.get(pane) {
            return value.clone();
        }
        let remaining = self.deadline.checked_duration_since(Instant::now())?;
        if remaining.is_zero() || self.cache.len() >= MAX_PANES {
            return None;
        }
        let value = self
            .herdr
            .clone()
            .with_timeout(remaining.min(REQUEST))
            .raw("pane.process_info", json!({"pane_id":pane}))
            .ok();
        self.cache.insert(pane.to_owned(), value.clone());
        value
    }
}

#[cfg(test)]
mod tests;
