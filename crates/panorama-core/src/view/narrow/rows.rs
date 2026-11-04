//! The narrow layout's root rows: an unselected one is a pod, its name and state, and its
//! marks; the selected one opens a card per direct subagent below it (Python `_narrow`, the
//! uniform tree of plan `uniform-agent-tree.md` §4).

use super::card::{CardCtx, agent_card, fin_card};
use super::links::join;
use crate::model::{Agent, HerdrStatus, Orchestrator, Status, Swarm, TreeKids};
use crate::view::cards::boxes::HOVER;
use crate::view::cards::marks::{marks_of, marks_right};
use crate::view::colour::Rgb;
use crate::view::nav::{find_agent, path_ids, tree_entries};
use crate::view::orch_state::{Ages, OrchState, orch_state};
use crate::view::palette::{DIM, FAINT, FG, HI, status};
use crate::view::pod::pod;
use crate::view::screen::Screen;
use crate::view::seg::Seg;
use crate::view::state::{FIN_CARD, View};
use crate::view::target::Target;
use crate::view::text::trunc;

/// The state words' colour: stale's, faint when quiet, else the state's.
fn state_colour(st: &OrchState) -> Rgb {
    if st.st == Status::Stale {
        status(&Status::Stale)
    } else if st.quiet() {
        FAINT
    } else {
        status(&st.st)
    }
}

/// An unselected orchestrator at row `y`: pod, name and state, marks on the right.
pub fn orch_row(cv: &mut Screen, y: i32, o: &Orchestrator, view: &View, ages: Ages) {
    let w = cv.w();
    let st = orch_state(o, view.frame, Some(ages));
    let target = Target::Orch(o.pane_id.clone());
    cv.area(0, w, y, y + 1, target.clone());
    if view.hover.as_ref() == Some(&target) {
        cv.put(0, y, "▏", HOVER, None, false);
    }
    pod(cv, 1, y, &st.st, &st.glyph, false);
    let agents: Vec<_> = o.agents().collect();
    let mk = marks_of(&agents, o.herdr_status == HerdrStatus::Blocked, 0);
    let mw = marks_right(cv, w - 2, y, &mk, Some(&o.pane_id));
    let name = trunc(
        &crate::view::harness::title(o, view.harness_icons),
        (w - 26 - mw).max(8),
    );
    let words = trunc(
        &format!("  {}", st.label),
        (w - 8 - name.chars().count() as i32 - mw).max(0),
    );
    cv.segs(
        5,
        y,
        &[
            Seg::new(name, if st.quiet() { DIM } else { FG }),
            Seg::new(words, state_colour(&st)),
        ],
    );
}

/// Where an opened orchestrator ended, and the rows of the selection in it.
pub struct Opened {
    pub end: i32,
    pub sel: Option<(i32, i32)>,
}

/// The selected root at row `y`: its line, then a card per direct subagent (and the fold of
/// its finished ones), in the depth-1 column's order, joined to it by lines.
pub fn open_orch(
    cv: &mut Screen,
    y: i32,
    o: &Orchestrator,
    swarm: &Swarm,
    view: &View,
    ages: Ages,
) -> Opened {
    let w = cv.w();
    let st = orch_state(o, view.frame, Some(ages));
    cv.area(0, w, y, y + 1, Target::Orch(o.pane_id.clone()));
    pod(cv, 1, y, &st.st, &st.glyph, false);
    let name = trunc(
        &crate::view::harness::title(o, view.harness_icons),
        (w - 40).max(8),
    );
    let segs = [
        Seg::new(name, HI).bold(),
        Seg::new(format!("  {}", st.label), state_colour(&st)),
    ];
    cv.segs(5, y, &segs);
    let mut sel = None;
    if view.focus == 0 {
        cv.put(0, y, "▌", HI, None, false);
        sel = Some((y, y + 1));
    }
    let kids = TreeKids::of_root(o);
    let below = |a: &Agent| {
        let mut out = vec![];
        descend(&kids, &a.id, 0, &mut out);
        out
    };
    // the depth-1 agent holding the selection (its fold card: the root's own)
    let picked = view.agent.as_deref().map(|id| {
        let id = id.strip_prefix(FIN_CARD).unwrap_or(id);
        path_ids(Some(o), Some(id)).first().cloned()
    });
    let ctx = CardCtx { o, swarm, view, w };
    let (top_y, mut y) = (y, y + 2);
    let mut anchors = vec![];
    for e in tree_entries(o, view).into_iter().filter(|e| e.depth == 1) {
        let placed = if e.ident.starts_with(FIN_CARD) {
            let fin: Vec<&Agent> = e
                .fin
                .iter()
                .filter_map(|id| find_agent(Some(o), Some(id)))
                .collect();
            fin_card(cv, y, &ctx, &fin, &e.ident)
        } else {
            let Some(a) = find_agent(Some(o), Some(&e.ident)) else {
                continue;
            };
            let open = picked
                .as_ref()
                .is_some_and(|p| p.as_deref() == Some(a.id.as_str()));
            let seed = anchors.len() as i64 + 1;
            agent_card(cv, y, &ctx, a, &below(a), (seed, open))
        };
        if placed.sel.is_some() {
            sel = placed.sel;
        }
        anchors.push(placed.anchor);
        y += placed.height;
    }
    join(cv, top_y, &anchors);
    Opened { end: y, sel }
}

/// Everything below agent `id`, depth first (a guard cuts it at 64 levels).
fn descend<'a>(kids: &TreeKids<'a>, id: &str, depth: usize, out: &mut Vec<&'a Agent>) {
    if depth > 64 {
        return;
    }
    for k in kids.children(Some(id)) {
        out.push(k);
        descend(kids, &k.id, depth + 1, out);
    }
}
