//! The docked panel's header line and footer (Python `render.dock_panel`'s first and last
//! lines): the orchestrator count and the swarm's `?N !N ✗N`; the rule, and the key hints
//! with herdr's open key filled in.

use crate::model::{Status, Swarm};
use crate::view::palette::{DIM, FAINT, KEY, LINE, ON_LIGHT, status};
use crate::view::screen::Screen;
use crate::view::seg::Seg;
use crate::view::state::View;

/// The hints, key then words (`DOCK_KEYS`).
const KEYS: [(&str, &str); 5] = [
    ("↑↓", " move "),
    ("ret", " open "),
    ("a", " answer "),
    ("n", " new "),
    (OPEN_KEY, " back"),
];
/// The open key's slot in [`KEYS`], filled from `View.open_key`.
const OPEN_KEY: &str = "pfx+spc";

/// Line 0: `SWARM  N orchestrators`, the refresh-failed badge, and the totals on the right.
pub fn header(s: &mut Screen, swarm: &Swarm, shown: usize, stale: bool) {
    let n = format!("{shown} orchestrators");
    s.segs(
        1,
        0,
        &[
            Seg::new("SWARM", DIM).bold(),
            Seg::new(format!("  {n}"), FAINT),
        ],
    );
    if stale {
        let x = 1 + 5 + 2 + n.chars().count() as i32 + 1;
        s.segs(
            x,
            0,
            &[Seg::new(" refresh failed ", ON_LIGHT)
                .bold()
                .on(status(&Status::Waiting))],
        );
    }
    let c = swarm.counts();
    s.segs_right(
        s.w - 1,
        0,
        &[
            Seg::new(format!("?{}", c.questions), status(&Status::Waiting)).bold(),
            Seg::plain(" "),
            Seg::new(format!("!{}", c.blocked), status(&Status::Blocked)).bold(),
            Seg::plain(" "),
            Seg::new(format!("✗{}", c.failed), status(&Status::Failed)).bold(),
        ],
    );
}

fn hints_width(keys: &[(String, &str)]) -> i32 {
    1 + keys
        .iter()
        .map(|(k, lab)| (k.chars().count() + lab.chars().count()) as i32)
        .sum::<i32>()
}

/// The hints with the open key filled in (the operator's rule, Q157): the key whole with
/// every hint; else whole without `n new`; only then its short form. Hints start at x=1 and
/// the last column stays free.
fn open_key_hints(
    keys: &[(&str, &'static str)],
    open_key: &(String, String),
    w: i32,
) -> Vec<(String, &'static str)> {
    let fill = |key: &str, skip_new: bool| -> Vec<(String, &'static str)> {
        keys.iter()
            .filter(|(k, _)| !(skip_new && *k == "n"))
            .map(|(k, lab)| {
                (
                    if *k == OPEN_KEY {
                        key.to_owned()
                    } else {
                        (*k).to_owned()
                    },
                    *lab,
                )
            })
            .collect()
    };
    for ks in [fill(&open_key.0, false), fill(&open_key.0, true)] {
        if hints_width(&ks) < w {
            return ks;
        }
    }
    fill(&open_key.1, false)
}

/// The last two lines: the rule (with the zoom hint in it) and the key hints. A narrow pane
/// drops hints whole, never cuts one.
pub fn footer(s: &mut Screen, view: &View, inbox_ok: bool) {
    let (w, h) = (s.w, s.h);
    s.put(0, h - 2, &"─".repeat(w.max(0) as usize), LINE, None, false);
    if view.zoom_hint {
        s.segs_right(
            w - 2,
            h - 2,
            &[
                Seg::new(" ", LINE),
                Seg::new("z", KEY),
                Seg::new(" graph ", FAINT),
            ],
        );
    }
    let keys: Vec<(&str, &'static str)> = KEYS
        .into_iter()
        .filter(|(k, _)| *k != "a" || inbox_ok)
        .collect();
    let mut keys = open_key_hints(&keys, &view.open_key, w);
    for drop in ["n", "a", "ret", "↑↓"] {
        if hints_width(&keys) <= w {
            break;
        }
        keys.retain(|(k, _)| k != drop);
    }
    let mut x = 1;
    for (k, lab) in keys {
        x = s.segs(x, h - 1, &[Seg::new(k, KEY), Seg::new(lab, FAINT)]);
    }
}
