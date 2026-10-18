//! `panorama snapshot` without `--json` (Python `cli.cmd_snapshot`): one line per
//! orchestrator with its background work, threads and agents; `--counts` prints the totals.

use super::json;
use super::pyjson::{J, dumps};
use panorama_core::model::{Status, Swarm};
use panorama_core::view::orch_state::{Ages, orch_state};
use panorama_core::view::text::ago;
use std::fmt::Write;

fn cut(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

/// The text listing.
pub fn listing(swarm: &Swarm) -> String {
    let bg_glyph = Status::Background.glyph();
    let ages = Ages {
        now: swarm.generated_at,
        stale_after: swarm.stale_after,
    };
    let mut out = String::new();
    for o in &swarm.orchestrators {
        let blocked = if o.herdr_status.code() == "blocked" {
            "!".to_owned()
        } else {
            String::new()
        };
        let asked = if o.inbox.is_empty() {
            String::new()
        } else {
            format!("?{}", o.inbox.len())
        };
        let marks = [blocked, asked]
            .into_iter()
            .filter(|m| !m.is_empty())
            .collect::<Vec<_>>()
            .join(" ");
        let st = orch_state(o, 0.0, Some(ages));
        let idle = o
            .last_activity
            .filter(|t| *t != 0.0)
            .map_or(String::new(), |t| {
                format!(" idle {}", ago(swarm.generated_at - t))
            });
        let _ = writeln!(
            out,
            "{}  [{}] {}{idle} {} {marks}",
            o.tab_label,
            o.herdr_status.code(),
            st.label,
            o.flavour
        );
        for b in &o.bg {
            let _ = writeln!(
                out,
                "  {bg_glyph} {:<8} {} {}",
                b.kind,
                b.id,
                cut(&b.description, 60)
            );
        }
        for t in &o.threads {
            let mark = if t.is_done() { "✓" } else { "•" };
            let (src, status) = (t.source.code(), t.status.code());
            let _ = writeln!(
                out,
                "  {mark} {}  ({src}, {status}, {} agents)",
                t.name,
                t.agents.len()
            );
            for a in &t.agents {
                let model = a.model.as_deref().unwrap_or("");
                let id = cut(&a.id, 10);
                let _ = writeln!(
                    out,
                    "      {} {:<9} {id} {model:<6} {}",
                    a.status.glyph(),
                    a.role,
                    cut(&a.activity, 60)
                );
                for b in &a.bg {
                    let _ = writeln!(
                        out,
                        "          {bg_glyph} {:<8} {} {}",
                        b.kind,
                        b.id,
                        cut(&b.description, 50)
                    );
                }
            }
        }
    }
    out
}

/// `--counts`: the totals, herdr and inbox state, threads by source, agents by status, hidden
/// threads and how long the collect took.
pub fn counts(swarm: &Swarm, seconds: f64) -> String {
    let mut out = json::counts(&swarm.counts());
    let mut by_source: Vec<(String, J)> = Vec::new();
    let mut by_status: Vec<(String, J)> = Vec::new();
    let bump =
        |items: &mut Vec<(String, J)>, key: &str| match items.iter_mut().find(|(k, _)| k == key) {
            Some((_, J::Int(n))) => *n += 1,
            _ => items.push((key.to_owned(), J::Int(1))),
        };
    for t in swarm.orchestrators.iter().flat_map(|o| &o.threads) {
        bump(&mut by_source, t.source.code());
        for a in &t.agents {
            bump(&mut by_status, a.status.code());
        }
    }
    let hidden: i64 = swarm.orchestrators.iter().map(|o| o.hidden_done).sum();
    out.extend([
        ("herdr_ok".to_owned(), J::Bool(swarm.herdr_ok)),
        ("inbox_ok".to_owned(), J::Bool(swarm.inbox_ok)),
        ("by_source".to_owned(), J::Obj(by_source)),
        ("agent_status".to_owned(), J::Obj(by_status)),
        ("hidden_done_threads".to_owned(), J::Int(hidden)),
        (
            "seconds".to_owned(),
            J::Float((seconds * 1000.0).round() / 1000.0),
        ),
    ]);
    dumps(&J::Obj(out), 2)
}
