//! An agent's role from its words (Python `threads.role_of`): secondary text on its card, and
//! a running reviewer reads "reviewing". A Workflow run's agents name it by their run label
//! first (plan `uniform-agent-tree.md` D8).

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

#[cfg(test)]
mod tests;
