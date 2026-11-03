use super::{MAX_CANDIDATES, MAX_THREADS, Scan};
use std::collections::{BTreeMap, BTreeSet, HashSet};

/// At most eight thread scans per root, matching the per-refresh metadata-read cap.
/// Root state is never evicted by a descendant; an exact different root has its own graph.
pub(in crate::codex) struct Graph {
    root: String,
    scans: BTreeMap<String, Scan>,
    proved: BTreeMap<String, String>,
    attempted: BTreeMap<(String, String), u64>,
    clock: u64,
}
impl Graph {
    pub fn new(root: &str) -> Self {
        Self {
            root: root.into(),
            scans: BTreeMap::new(),
            proved: BTreeMap::new(),
            attempted: BTreeMap::new(),
            clock: 0,
        }
    }
    pub fn root(&mut self, id: &str) {
        if self.root != id {
            self.root = id.into();
            self.scans.clear();
            self.proved.clear();
            self.attempted.clear();
            self.clock = 0;
        }
    }
    /// Hints select work only. Query revalidates the full ancestor chain before use.
    /// An unread deep target beats already attempted foreign references at the same cap.
    pub fn focus(&self) -> Vec<(String, String)> {
        let mut edges: BTreeSet<_> = self
            .scans
            .iter()
            .flat_map(|(parent, scan)| {
                scan.children
                    .iter()
                    .map(move |child| (parent.clone(), child.clone()))
            })
            .collect();
        edges.extend(
            self.proved
                .iter()
                .map(|(child, parent)| (parent.clone(), child.clone())),
        );
        edges
            .into_iter()
            .filter(|(parent, child)| child != &self.root && parent != child)
            .filter_map(|(parent, child)| {
                let edge = (parent.clone(), child.clone());
                let mut path = vec![(parent.clone(), child.clone())];
                let mut seen = HashSet::from([child.clone()]);
                let mut ancestor = parent;
                while ancestor != self.root {
                    if !seen.insert(ancestor.clone()) || path.len() >= MAX_THREADS - 1 {
                        return None;
                    }
                    let parent = self.proved.get(&ancestor)?.clone();
                    path.push((parent.clone(), ancestor));
                    ancestor = parent;
                }
                path.reverse();
                Some((
                    self.attempted.get(&edge).copied().unwrap_or(0),
                    std::cmp::Reverse(path.len()),
                    child,
                    path,
                ))
            })
            .min_by(|a, b| (&a.0, &a.1, &a.2).cmp(&(&b.0, &b.1, &b.2)))
            .map_or_else(Vec::new, |value| value.3)
    }
    pub fn attempt(&mut self, parent: &str, child: &str) {
        let edge = (parent.to_owned(), child.to_owned());
        const MAX_ATTEMPTS: usize = MAX_THREADS * MAX_CANDIDATES;
        if self.attempted.len() >= MAX_ATTEMPTS
            && !self.attempted.contains_key(&edge)
            && let Some(old) = self
                .attempted
                .iter()
                .min_by_key(|(_, time)| *time)
                .map(|(id, _)| id.clone())
        {
            self.attempted.remove(&old);
        }
        self.clock = self.clock.saturating_add(1);
        self.attempted.insert(edge, self.clock);
    }
    /// Previous proof is a scheduling hint only; every returned thread is still reread.
    /// Rotate unverified candidates so a prefix of foreign IDs cannot starve siblings.
    pub fn order(&mut self, parent: &str, ids: Vec<String>) -> Vec<String> {
        let (mut known, mut pending): (Vec<_>, Vec<_>) = ids
            .into_iter()
            .partition(|id| self.proved.get(id).is_some_and(|p| p == parent));
        if !pending.is_empty() {
            let scan = self.scan(parent);
            let offset = scan.rotation % pending.len();
            pending.rotate_left(offset);
            if known.len() >= MAX_THREADS - 1 {
                let offset = scan.rotation % known.len();
                known.rotate_left(offset);
                // Keep one of the seven child-read slots available for discovery.
                let rest = known.split_off(MAX_THREADS - 2);
                known.push(pending.remove(0));
                known.extend(rest);
            }
            scan.rotation = scan.rotation.wrapping_add(1);
        } else if known.len() > MAX_THREADS - 1 {
            let scan = self.scan(parent);
            let offset = scan.rotation % known.len();
            known.rotate_left(offset);
            scan.rotation = scan.rotation.wrapping_add(MAX_THREADS - 1);
        }
        known.extend(pending);
        known
    }
    pub fn proof(&mut self, parent: &str, child: &str, valid: bool) {
        if valid && (self.proved.len() < MAX_CANDIDATES || self.proved.contains_key(child)) {
            self.proved.insert(child.into(), parent.into());
        } else if self.proved.get(child).is_some_and(|p| p == parent) {
            // A rejected reference from another parent cannot erase a valid edge hint.
            self.proved.remove(child);
        }
    }
    pub fn scan(&mut self, id: &str) -> &mut Scan {
        if self.scans.len() >= MAX_THREADS
            && !self.scans.contains_key(id)
            && let Some(old) = self.scans.keys().find(|key| **key != self.root).cloned()
        {
            self.scans.remove(&old);
        }
        self.scans.entry(id.into()).or_default()
    }
}
