use super::*;
use crate::machines::tests::SID;

fn ids(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| (*s).to_owned()).collect()
}

#[test]
fn malformed_session_ids_never_reach_rsync() {
    let rules = rules(&ids(&["", "..", "../etc", SID]), Kind::Jsonl);
    assert!(
        rules
            .iter()
            .filter(|r| r.starts_with("--include=*/") && *r != "--include=*/")
            .all(|r| r.contains(SID))
    );
    assert_eq!(rules.len(), 6);
    assert_eq!(
        super::rules(&ids(&["", ".."]), Kind::Jsonl),
        ["--include=*/", "--exclude=*"]
    );
}

#[test]
fn each_pass_names_its_own_files() {
    assert_eq!(
        rules(&ids(&[SID]), Kind::Json),
        [
            "--include=*/".to_owned(),
            format!("--include=*/{SID}/subagents/agent-*.meta.json"),
            format!("--include=*/{SID}/subagents/workflows/*/agent-*.meta.json"),
            format!("--include=*/{SID}/workflows/*.json"),
            "--exclude=*".to_owned(),
        ]
    );
}
