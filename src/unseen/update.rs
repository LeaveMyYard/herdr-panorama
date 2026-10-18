//! One refresh against the state file (Python `Tracker.update`, `_update`, `key`).

use super::rules::{event, float, mark, qids, record, urgent};
use super::{Event, KEEP_SECONDS, Marks, Tracker, store};
use crate::snapshot::pyjson::J;
use panorama_core::model::{Agent, Orchestrator, Status, Swarm};
use std::collections::{BTreeMap, BTreeSet};

/// The state file's key: an agent on another herdr machine carries that machine's id.
pub fn key(o: &Orchestrator, a: &Agent) -> String {
    let set = |s: &Option<String>| s.clone().filter(|s| !s.is_empty());
    match set(&o.machine_id).or_else(|| set(&o.machine)) {
        Some(mid) => format!("{mid}:{}", a.id),
        None => a.id.clone(),
    }
}

/// Python's `a.ended or a.last_at or a.started or 0.0`.
fn when(a: &Agent) -> f64 {
    [a.ended, a.last_at, a.started]
        .into_iter()
        .flatten()
        .find(|t| *t != 0.0)
        .unwrap_or(0.0)
}

impl Tracker {
    /// Record this refresh's agents at `now`; the unseen marks. Runs on the collect worker:
    /// the file IO and the lock wait stay off the UI thread. A busy lock or an IO error keeps
    /// the last marks.
    pub fn update(&self, swarm: &Swarm, now: f64) -> Marks {
        if !(self.enabled)() {
            self.inner().marks.clear();
            return Marks::new();
        }
        let fresh = store::locked(&self.path, || self.update_locked(swarm, now))
            .ok()
            .flatten();
        let mut inner = self.inner();
        if let Some(marks) = fresh {
            inner.marks = marks;
        }
        inner.marks.clone()
    }

    fn update_locked(&self, swarm: &Swarm, now: f64) -> std::io::Result<Marks> {
        let read = store::read(&self.path);
        let first = read.is_none();
        let mut data = read.unwrap_or_else(|| {
            J::obj([
                ("v", J::Int(1)),
                ("saved", J::Float(now)),
                ("agents", J::Obj(Vec::new())),
            ])
        });
        let saved = float(data.get("saved"));
        // focus clears an orchestrator's marks only when it MOVES onto that pane: focus resting
        // on a pane is not looking, and the first refresh after start has nothing to compare with
        let focused: BTreeSet<String> = swarm
            .orchestrators
            .iter()
            .filter(|o| o.focused && o.machine.is_none())
            .map(|o| o.pane_id.clone())
            .collect();
        let (arrived, pending) = {
            let mut inner = self.inner();
            let arrived = inner
                .prev_focused
                .as_ref()
                .map_or_else(BTreeSet::new, |p| &focused - p);
            inner.prev_focused = Some(focused);
            (arrived, inner.pending.clone())
        };
        let mut agents = match data.get_mut("agents") {
            Some(J::Obj(items)) => std::mem::take(items),
            _ => Vec::new(),
        };
        let mut keymap = BTreeMap::new();
        for o in &swarm.orchestrators {
            for a in o.agents().filter(|a| !a.id.starts_with("main:")) {
                let k = key(o, a);
                keymap.insert(a.id.clone(), k.clone());
                let at = agents
                    .iter()
                    .position(|(key, rec)| *key == k && rec.is_obj());
                let ev = match at {
                    Some(i) => {
                        let rec = &agents[i].1;
                        event(
                            rec.get("s").and_then(J::as_str),
                            &qids(rec),
                            &a.status,
                            &a.question_ids,
                        )
                    }
                    None => (!first && when(a) >= saved)
                        .then(|| event(None, &[], &a.status, &a.question_ids))
                        .flatten(),
                };
                let i = at.unwrap_or_else(|| {
                    // a new agent (or a record that is not one: replaced where it was)
                    let rec = record(&a.status, &a.question_ids, now);
                    match agents.iter().position(|(key, _)| *key == k) {
                        Some(i) => {
                            agents[i].1 = rec;
                            i
                        }
                        None => {
                            agents.push((k.clone(), rec));
                            agents.len() - 1
                        }
                    }
                });
                let rec = &mut agents[i].1;
                let mut prev = mark(rec);
                if prev == Some(Event::Wait)
                    && a.question_ids.is_empty()
                    && a.status != Status::Waiting
                {
                    prev = None; // answered: seen where it was answered
                }
                if arrived.contains(&o.pane_id) || pending.contains(&a.id) {
                    prev = None; // focus moved onto its orchestrator's pane, or seen here
                }
                let ev = urgent(prev, ev);
                let mut q: BTreeSet<i64> = qids(rec).into_iter().collect();
                q.extend(a.question_ids.iter().copied());
                rec.set("ev", ev.map_or(J::Null, |e| J::str(e.code())));
                rec.set("s", J::str(a.status.code()));
                rec.set("q", J::ints(q));
                rec.set("t", J::Float(now));
            }
        }
        agents.retain(|(_, r)| now - float(r.get("t")) <= KEEP_SECONDS);
        let marks: Marks = keymap
            .iter()
            .filter_map(|(aid, k)| {
                agents
                    .iter()
                    .find(|(key, _)| key == k)
                    .and_then(|(_, r)| mark(r))
                    .map(|e| (aid.clone(), e))
            })
            .collect();
        data.set("saved", J::Float(now));
        data.set("agents", J::Obj(agents));
        store::write(&self.path, &data)?;
        let mut inner = self.inner();
        for i in &pending {
            inner.pending.remove(i);
        }
        inner.keymap = keymap;
        Ok(marks)
    }
}
