//! `panorama emit`'s phase and agent specs (Python `cli.cmd_emit`), as the channel's records.

use crate::snapshot::pyjson::J;

/// `"name=state[@start[-end]][:note]"` → a phase (state `next` when empty).
pub fn phase(spec: &str) -> J {
    let (name, rest) = spec.split_once('=').unwrap_or((spec, ""));
    let (state, note) = rest.split_once(':').unwrap_or((rest, ""));
    let (state, times) = state.split_once('@').unwrap_or((state, ""));
    let (start, end) = times.split_once('-').unwrap_or((times, ""));
    let mut p = vec![
        ("name".to_owned(), J::str(name)),
        (
            "state".to_owned(),
            J::str(if state.is_empty() { "next" } else { state }),
        ),
    ];
    for (k, v) in [("start", start), ("end", end), ("note", note)] {
        if !v.is_empty() {
            p.push((k.to_owned(), J::str(v)));
        }
    }
    J::Obj(p)
}

/// `"match[=role[@phase]]"` → an agent spec (the phase only when it is all digits).
pub fn agent(spec: &str) -> J {
    let (pattern, rest) = spec.split_once('=').unwrap_or((spec, ""));
    let (role, ph) = rest.split_once('@').unwrap_or((rest, ""));
    let mut a = vec![("match".to_owned(), J::str(pattern))];
    if !role.is_empty() {
        a.push(("role".to_owned(), J::str(role)));
    }
    // Python's `str.isdigit()` then `int()`: ASCII digits here (stated in `emit`)
    if !ph.is_empty()
        && ph.bytes().all(|b| b.is_ascii_digit())
        && let Ok(n) = ph.parse::<i64>()
    {
        a.push(("phase".to_owned(), J::Int(n)));
    }
    J::Obj(a)
}

#[cfg(test)]
mod tests;
