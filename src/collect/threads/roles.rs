//! An agent's role from its words, and the PR or issue its description names (Python
//! `threads.role_of`, `ref_key`).

use regex::Regex;
use std::sync::OnceLock;

/// Role keywords, in priority order for a tie at one position.
const ROLE_WORDS: [(&str, &[&str]); 6] = [
    ("reviewer", &["review", "verif"]),
    ("fixer", &["fix", "cifix"]),
    ("merger", &["merge"]),
    (
        "author",
        &["author", "implement", "build", "write", "draft", "rebase"],
    ),
    (
        "explorer",
        &[
            "explor",
            "research",
            "investigat",
            "find",
            "map",
            "search",
            "survey",
            "read",
            "look",
        ],
    ),
    ("planner", &["plan"]),
];

/// Words before a reference that are not a repository name.
const STOP: [&str; 35] = [
    "review",
    "re-review",
    "rereview",
    "merge",
    "fix",
    "fixes",
    "rebase",
    "author",
    "for",
    "of",
    "on",
    "in",
    "the",
    "and",
    "to",
    "pr",
    "issue",
    "check",
    "ci",
    "a",
    "an",
    "re",
    "reviewer",
    "close",
    "author-rebase",
    "with",
    "from",
    "after",
    "before",
    "about",
    "is",
    "at",
    "by",
    "into",
    "draft",
];

/// The word a role's phases are named by ("review r2", "author 2").
pub fn phase_word(role: &str) -> &str {
    match role {
        "reviewer" => "review",
        "fixer" => "fix",
        "author" => "author",
        "explorer" => "explore",
        "merger" => "merge",
        "planner" => "plan",
        other => other,
    }
}

/// `re.search(r"(?<![a-z])" + word, low)`: the first match's start.
fn word_at(low: &str, word: &str) -> Option<usize> {
    low.match_indices(word).map(|(i, _)| i).find(|&i| {
        !low[..i]
            .chars()
            .next_back()
            .is_some_and(|c| c.is_ascii_lowercase())
    })
}

/// The first role keyword found in the texts, in order (earliest position wins in a text);
/// else `fallback`, else "agent".
pub fn role_of(texts: &[&str], fallback: Option<&str>) -> String {
    for text in texts {
        let low = text.to_lowercase();
        let mut best: Option<(usize, &str)> = None;
        for (role, words) in ROLE_WORDS {
            for w in words {
                if let Some(at) = word_at(&low, w)
                    && best.is_none_or(|(b, _)| at < b)
                {
                    best = Some((at, role));
                }
            }
        }
        if let Some((_, role)) = best {
            return role.into();
        }
    }
    fallback.unwrap_or("agent").into()
}

/// `(key, display name)` of the first PR or issue reference in a description:
/// `("repo#12", "Repo #12")`, or `("#12", "#12")` without a repository.
pub fn ref_key(description: &str) -> Option<(String, String)> {
    static R: OnceLock<Regex> = OnceLock::new();
    let re = R.get_or_init(|| {
        Regex::new(r"(?i)(?:\b([A-Za-z][\w.-]*)\s+)?(?:\bPR\s*#?\s*|pull/|#)(\d{1,6})\b")
            .expect("the ref pattern")
    });
    let m = re.captures(description)?;
    let num = m.get(2)?.as_str();
    let repo = m.get(1).map(|r| r.as_str()).filter(|r| {
        !STOP.contains(&r.to_lowercase().as_str()) && !r.chars().all(|c| c.is_ascii_digit())
    });
    Some(match repo {
        Some(r) => (format!("{}#{num}", r.to_lowercase()), format!("{r} #{num}")),
        None => (format!("#{num}"), format!("#{num}")),
    })
}

#[cfg(test)]
mod tests;
