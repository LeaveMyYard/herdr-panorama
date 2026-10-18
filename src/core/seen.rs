//! A read's `seen` ids, each once, in order (Python `dict.fromkeys`): linear, as the reader
//! itself is, so a long `seen` line never stalls the core's one thread. At most [`MAX_SEEN`]
//! ids go to the app per read: a real list is a handful of agent ids, and up to
//! [`super::EVENTS`] events can wait for the app at once.

use std::collections::HashSet;
use std::hash::Hash;

/// The ids one read hands to the app; the rest of a longer list is dropped (and noted).
pub const MAX_SEEN: usize = 1024;

/// `ids` without repeats, first occurrence first, at most `cap` of them; whether any were
/// dropped over the cap.
pub fn once_each<T: Eq + Hash + Clone>(ids: Vec<T>, cap: usize) -> (Vec<T>, bool) {
    let mut set = HashSet::with_capacity(ids.len().min(cap));
    let mut out = Vec::new();
    let mut over = false;
    for id in ids {
        if set.contains(&id) {
            continue;
        }
        if out.len() == cap {
            over = true;
            break;
        }
        set.insert(id.clone());
        out.push(id);
    }
    (out, over)
}

#[cfg(test)]
mod tests;
