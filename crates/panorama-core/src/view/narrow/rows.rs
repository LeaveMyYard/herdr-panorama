//! The narrow layout's orchestrator rows: an unselected one is a pod, its name and state,
//! and its marks; the selected one opens its thread cards below it (Python `_narrow`).

use super::card::{CardCtx, done_card, thread_card};
use super::links::join;
use crate::model::ThreadSource;
use crate::model::{HerdrStatus, Orchestrator, Status, Swarm, Thread};
use crate::view::cards::boxes::HOVER;
use crate::view::cards::marks::{marks_of, marks_right};
use crate::view::colour::Rgb;
use crate::view::filter::Filter;
use crate::view::nav::find_agent;
use crate::view::orch_state::{Ages, OrchState, orch_state};
use crate::view::palette::{DIM, FAINT, FG, HI, status};
use crate::view::pod::pod;
use crate::view::screen::Screen;
use crate::view::seg::Seg;
use crate::view::state::View;
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

/// The selected orchestrator at row `y`: its line, then its thread cards (and the done
/// card), joined to it by lines.
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
        Seg::new(format!("  {}", o.flavour), FAINT),
    ];
    cv.segs(5, y, &segs);
    let mut sel = None;
    if view.focus == 0 {
        cv.put(0, y, "▌", HI, None, false);
        sel = Some((y, y + 1));
    }
    let (active, done) = split_threads(o, view.filter);
    let mut cards: Vec<Option<&Thread>> = active.iter().map(|t| Some(*t)).collect();
    if !done.is_empty() {
        cards.push(None);
    }
    // interim, until U5 replaces narrow: the thread holding the selected agent opens, else
    // the first
    let picked = find_agent(Some(o), view.agent.as_deref());
    let open = active
        .iter()
        .position(|t| picked.is_some_and(|a| t.agents.iter().any(|b| b.id == a.id)))
        .unwrap_or(0);
    let ctx = CardCtx { o, swarm, view, w };
    let (top_y, mut y) = (y, y + 2);
    let mut anchors = vec![];
    for (i, card) in cards.into_iter().enumerate() {
        let placed = match card {
            None => done_card(cv, y, &ctx, &done),
            Some(t) => thread_card(cv, y, &ctx, t, (anchors.len() as i64 + 1, i == open)),
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

/// An orchestrator's threads as narrow shows them (interim, the former `nav::split_threads`,
/// until U5 replaces narrow): the active ones (a failed one stays with them), and the done
/// ones it folds.
fn split_threads(o: &Orchestrator, filter: Filter) -> (Vec<&Thread>, Vec<&Thread>) {
    use crate::model::ThreadStatus;
    let is_active = |t: &Thread| {
        (!t.is_done() && t.status != ThreadStatus::Failed) || t.source == ThreadSource::Session
    };
    let threads = || {
        o.threads
            .iter()
            .filter(|t| !t.is_tree() || !t.agents.is_empty())
    };
    let mut active: Vec<&Thread> = threads().filter(|t| is_active(t)).collect();
    let rest = threads().filter(|t| !is_active(t));
    let (failed, done): (Vec<&Thread>, Vec<&Thread>) =
        rest.partition(|t| t.status == ThreadStatus::Failed);
    active.extend(failed);
    match filter {
        Filter::Needs => {
            let needs = |t: &&Thread| {
                t.agents.iter().any(|a| a.status.needs_operator())
                    || t.status == ThreadStatus::Failed
            };
            (active.into_iter().filter(needs).collect(), vec![])
        }
        Filter::Running => (
            active
                .into_iter()
                .filter(|t| t.status == ThreadStatus::Active)
                .collect(),
            vec![],
        ),
        Filter::All => (active, done),
    }
}
