//! The collector's text rules (Python `collect.short_model`, `dur`, `_paragraph`,
//! `INBOX_MARK`).

use super::py::{chars, py_isspace};

const FAMILIES: [&str; 4] = ["opus", "sonnet", "haiku", "fable"];

/// The ends of a run of 1..=`max` ASCII digits at `i`, longest first (a greedy regex group).
fn digits_at(b: &[u8], i: usize, max: usize) -> Vec<usize> {
    let mut n = 0;
    while n < max && b.get(i + n).is_some_and(u8::is_ascii_digit) {
        n += 1;
    }
    (1..=n).rev().map(|k| i + k).collect()
}

fn is_in(b: &[u8], i: usize, set: &[u8]) -> bool {
    b.get(i).is_some_and(|c| set.contains(c))
}

/// `(opus|sonnet|haiku|fable)[-_ ]?(\d{1,2})(?:[-_.](\d{1,2}))?(?!\d)`: "opus 4.5".
fn family_first(low: &str) -> Option<String> {
    let b = low.as_bytes();
    let not_digit = |i: usize| !b.get(i).is_some_and(u8::is_ascii_digit);
    // a model name is untrusted text: only char boundaries start a match (never a byte slice
    // through a character)
    for i in (0..b.len()).filter(|&i| low.is_char_boundary(i)) {
        for fam in FAMILIES.iter().filter(|f| low[i..].starts_with(**f)) {
            let after = i + fam.len();
            let seps = if is_in(b, after, b"-_ ") {
                vec![after + 1, after]
            } else {
                vec![after]
            };
            for s in seps {
                for e1 in digits_at(b, s, 2) {
                    if is_in(b, e1, b"-_.")
                        && let Some(e2) =
                            digits_at(b, e1 + 1, 2).into_iter().find(|&e| not_digit(e))
                    {
                        return Some(format!("{fam} {}.{}", &low[s..e1], &low[e1 + 1..e2]));
                    }
                    if not_digit(e1) {
                        return Some(format!("{fam} {}", &low[s..e1]));
                    }
                }
            }
        }
    }
    None
}

/// `(?<!\d)(\d{1,2})(?:[-_.](\d{1,2}))?[-_ ](opus|sonnet|haiku|fable)`: "sonnet 3.5".
fn version_first(low: &str) -> Option<String> {
    let b = low.as_bytes();
    let fam_at = |i: usize| {
        FAMILIES
            .into_iter()
            .find(|f| low.get(i..).is_some_and(|r| r.starts_with(f)))
    };
    for i in 0..b.len() {
        if i > 0 && b[i - 1].is_ascii_digit() {
            continue;
        }
        for e1 in digits_at(b, i, 2) {
            let major = &low[i..e1];
            if is_in(b, e1, b"-_.") {
                for e2 in digits_at(b, e1 + 1, 2) {
                    if is_in(b, e2, b"-_ ")
                        && let Some(fam) = fam_at(e2 + 1)
                    {
                        return Some(format!("{fam} {major}.{}", &low[e1 + 1..e2]));
                    }
                }
            }
            if is_in(b, e1, b"-_ ")
                && let Some(fam) = fam_at(e1 + 1)
            {
                return Some(format!("{fam} {major}"));
            }
        }
    }
    None
}

/// The model's family and version: "claude-opus-4-5-20251101" → "opus 4.5",
/// "claude-3-5-sonnet-20241022" → "sonnet 3.5"; the family alone without a version; the
/// model itself when no family is named.
pub fn short_model(model: Option<&str>) -> Option<String> {
    let model = model.filter(|m| !m.is_empty())?;
    let low = model.to_lowercase();
    family_first(&low)
        .or_else(|| version_first(&low))
        .or_else(|| {
            FAMILIES
                .into_iter()
                .find(|f| low.contains(f))
                .map(str::to_owned)
        })
        .or_else(|| Some(model.to_owned()))
}

/// The first paragraph, its whitespace collapsed, at most 400 characters.
pub fn paragraph(text: &str) -> String {
    const LIMIT: usize = 400;
    let first = text
        .trim_matches(py_isspace)
        .split("\n\n")
        .next()
        .unwrap_or("");
    let para = first
        .split(py_isspace)
        .filter(|w| !w.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    if para.chars().count() <= LIMIT {
        para
    } else {
        format!("{}…", chars(&para, LIMIT - 1))
    }
}

/// A tab label without herdr-inbox's `❓N ` prefixes (`^(?:❓\d+\s+)+`): the card shows `?N`.
pub fn strip_inbox_mark(label: &str) -> &str {
    let mut rest = label;
    loop {
        let Some(after) = rest.strip_prefix('❓') else {
            return rest;
        };
        let digits = after.len() - after.trim_start_matches(|c: char| c.is_ascii_digit()).len();
        let spaced = after[digits..].trim_start_matches(char::is_whitespace);
        if digits == 0 || spaced.len() == after.len() - digits {
            return rest;
        }
        rest = spaced;
    }
}

#[cfg(test)]
mod tests;
