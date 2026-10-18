//! Reading the inbox's answers (Python `inbox_link._version`, `_BEFORE_POPUP`).

/// The first version with the deep link.
pub const DEEP_LINK: (u64, u64, u64) = (0, 6, 0);

/// `major.minor[.patch]` at the start of `text` (Python's `(\d+)\.(\d+)(?:\.(\d+))?`
/// matched at the start); `(0, 0, 0)` for anything else.
pub fn version(text: &str) -> (u64, u64, u64) {
    let num = |s: &str| -> Option<(u64, usize)> {
        let n = s.bytes().take_while(u8::is_ascii_digit).count();
        (n > 0)
            .then(|| (s[..n].parse().ok(), n))
            .and_then(|(v, n)| Some((v?, n)))
    };
    let Some((major, a)) = num(text) else {
        return (0, 0, 0);
    };
    let Some(rest) = text[a..].strip_prefix('.') else {
        return (0, 0, 0);
    };
    let Some((minor, b)) = num(rest) else {
        return (0, 0, 0);
    };
    let patch = rest[b..]
        .strip_prefix('.')
        .and_then(num)
        .map_or(0, |(p, _)| p);
    (major, minor, patch)
}

/// Words in a failed deep link's output that mean it failed before any popup opened (so the
/// plain popup may open instead).
pub fn before_popup(out: &str) -> bool {
    let out = out.to_lowercase();
    [
        "could not open the inbox popup",
        "unrecognized arguments",
        "invalid choice",
        "unable to find lockfile",
        "no such file or directory",
        "failed to spawn",
    ]
    .iter()
    .any(|w| out.contains(w))
}
