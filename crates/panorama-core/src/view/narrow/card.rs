//! The narrow layout's cards under the selected orchestrator (Python `_narrow`): a thread's
//! card (opened when it is the selected thread: its phases and its selected agent's
//! details), and the "n complete" card for the done threads.

use crate::model::{Agent, Orchestrator, Status, Swarm, Thread};
use crate::view::agent_text::{age_of, role_label, short_id};
use crate::view::cards::Area;
use crate::view::cards::actions::actions;
use crate::view::cards::boxes::{border, rbox};
use crate::view::cards::details::agent_detail_lines;
use crate::view::cards::edge::edge;
use crate::view::cards::marks::{marks_of, marks_right};
use crate::view::cards::phases::{phase_sub, step_line, thread_glyph};
use crate::view::cards::pods::{Pods, pods_row};
use crate::view::nav::find_agent;
use crate::view::orch_state::{Extras, state_of};
use crate::view::palette::{DIM, FAINT, FG, HI, status};
use crate::view::pod::apod;
use crate::view::screen::Screen;
use crate::view::seg::Seg;
use crate::view::state::{DONE_CARD, View};
use crate::view::target::{AgentRef, Target};
use crate::view::text::trunc;

/// What every card under one orchestrator shares.
pub struct CardCtx<'a> {
    pub o: &'a Orchestrator,
    pub swarm: &'a Swarm,
    pub view: &'a View,
    pub w: i32,
}

/// A card as placed: its height, the row its link joins, and the selected rows in it.
pub struct Placed {
    pub height: i32,
    pub anchor: i32,
    pub sel: Option<(i32, i32)>,
}

/// The done threads folded into one line: `✓ n complete · names`.
pub fn done_card(cv: &mut Screen, y: i32, c: &CardCtx, done: &[&Thread]) -> Placed {
    let card = format!("{DONE_CARD}{}", c.o.pane_id);
    cv.area(3, c.w - 1, y, y + 1, Target::Done(c.o.pane_id.clone()));
    let names = done
        .iter()
        .map(|t| t.name.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    let text = trunc(&format!(" {} complete · {names}", done.len()), c.w - 9);
    cv.segs(
        5,
        y,
        &[
            Seg::new("✓", status(&Status::Done)).bold(),
            Seg::new(text, FAINT),
        ],
    );
    let mut sel = None;
    if c.view.thread.as_deref() == Some(card.as_str()) {
        let focused = c.view.focus == 1;
        cv.put(3, y, "▸", if focused { HI } else { DIM }, None, false);
        if focused {
            sel = Some((y, y + 1));
        }
    }
    Placed {
        height: 1,
        anchor: y,
        sel,
    }
}

/// A thread's card at row `y`; `seed` shifts its edge's sweep. The selected thread's card
/// is opened: its phases, then its selected agent.
pub fn thread_card(cv: &mut Screen, y: i32, c: &CardCtx, t: &Thread, seed: i64) -> Placed {
    let view = c.view;
    let exp = view.thread.as_deref() == Some(t.id.as_str());
    let agent = if exp {
        find_agent(Some(t), view.agent.as_deref())
    } else {
        None
    };
    let det = agent.map_or_else(Vec::new, |a| {
        agent_detail_lines(a, c.w - 10, view.now, view.frame)
    });
    let steps = t.phases.len() as i32;
    let height = match (exp, agent) {
        (true, Some(_)) => 4 + 1 + steps + 1 + 1 + det.len() as i32 + 1 + 1,
        (true, None) => 4 + 1 + steps + 1,
        _ => 4,
    };
    frame(
        cv,
        Area {
            x: 3,
            y,
            w: c.w - 4,
            h: height,
        },
        c,
        t,
        exp,
        seed,
    );
    let mut sel = None;
    if exp {
        let r = phases(cv, y + 4, t, view, c.w);
        if let Some(a) = agent {
            agent_block(cv, r, c, t, a, &det);
        }
        if view.focus >= 1 {
            sel = Some((y, y + height));
        }
    }
    Placed {
        height,
        anchor: y + 1,
        sel,
    }
}

/// The card's box and edge, its title line (glyph, name, marks) and its pods and subtitle.
fn frame(cv: &mut Screen, card: Area, c: &CardCtx, t: &Thread, exp: bool, seed: i64) {
    let (view, w, y) = (c.view, c.w, card.y);
    let target = Target::Thread {
        pane: c.o.pane_id.clone(),
        thread: t.id.clone(),
    };
    cv.area(3, w - 1, y, y + card.h, target.clone());
    rbox(
        cv,
        card.x,
        y,
        card.w,
        card.h,
        border(view, &target, exp && view.focus == 1, exp),
    );
    let agents: Vec<_> = t.agents.iter().collect();
    let state = state_of(&agents, t.is_done(), view.frame, &Extras::default(), None);
    edge(cv, card, &state, view.frame, seed);
    let mw = marks_right(cv, w - 3, y + 1, &marks_of(&agents, false, 0), None);
    let (g, gc) = thread_glyph(t, view);
    let name = Seg::new(trunc(&t.name, w - 13 - mw), if exp { HI } else { FG }).bold();
    cv.segs(5, y + 1, &[Seg::new(g, gc).bold(), Seg::plain(" "), name]);
    let sel_agent = if exp { view.agent.as_deref() } else { None };
    pods_row(
        cv,
        7,
        y + 2,
        t,
        view,
        Pods {
            max_n: 6,
            sel_agent,
            orch: Some(&c.o.pane_id),
            top_only: false,
        },
    );
    let sub = trunc(&phase_sub(t, view), w - 40);
    cv.segs(
        w - 3 - sub.chars().count() as i32,
        y + 2,
        &[Seg::new(sub, DIM)],
    );
}

/// The opened card's phases from row `r`, a line each; the row after them and a gap.
fn phases(cv: &mut Screen, mut r: i32, t: &Thread, view: &View, w: i32) -> i32 {
    for i in 0..t.phases.len() {
        step_line(cv, 7, r, t, i, w - 12, view);
        r += 1;
    }
    r + 1
}

/// The selected agent at row `r`: its pod, role, id, model, age and calls, its detail lines
/// `det`, and its action hints.
fn agent_block(cv: &mut Screen, mut r: i32, c: &CardCtx, t: &Thread, a: &Agent, det: &[Vec<Seg>]) {
    let (view, w) = (c.view, c.w);
    let ident = AgentRef {
        pane: c.o.pane_id.clone(),
        thread: t.id.clone(),
        agent: a.id.clone(),
    };
    cv.area(
        4,
        w - 2,
        r,
        r + det.len() as i32 + 3,
        Target::Agent(ident.clone()),
    );
    apod(cv, 7, r, a, view.frame, false);
    let model = a
        .model
        .as_deref()
        .filter(|m| !m.is_empty())
        .map_or(String::new(), |m| format!(" · {m}"));
    let age = age_of(a, view.now).unwrap_or_else(|| "-".into());
    let role = role_label(a);
    let about = format!(" {}{model} · {age} · {} calls", short_id(a), a.tool_calls);
    let room = w - 16 - role.chars().count() as i32;
    cv.segs(
        11,
        r,
        &[
            Seg::new(role, HI).bold(),
            Seg::new(trunc(&about, room), DIM),
        ],
    );
    r += 1;
    for line in det {
        cv.segs(7, r, line);
        r += 1;
    }
    actions(cv, 7, r + 1, a, c.swarm.inbox_ok, Some(&ident), None);
}
