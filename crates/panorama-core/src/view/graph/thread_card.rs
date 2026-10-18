//! The wide graph's thread cards (Python `render.thread_card`, `bg_card`): a thread with its
//! state, phase and top-level pods (opened into its phases when selected), and the card of
//! the orchestrator's own background work above them.

use super::column::Place;
use crate::model::{Orchestrator, Status, Thread};
use crate::view::anim::clock;
use crate::view::cards::Area;
use crate::view::cards::boxes::{border, rbox};
use crate::view::cards::details::bg_row;
use crate::view::cards::edge::edge;
use crate::view::cards::marks::{marks_of, marks_right};
use crate::view::cards::phases::{phase_sub, step_line, thread_glyph};
use crate::view::cards::pods::{Pods, pods_row};
use crate::view::cards::pulse::pulse_box;
use crate::view::cards::tags::tags;
use crate::view::orch_state::{Extras, OrchState, state_of};
use crate::view::palette::{DIM, FG, HI, status};
use crate::view::screen::Screen;
use crate::view::seg::Seg;
use crate::view::state::{BG_CARD, View};
use crate::view::target::Target;
use crate::view::text::{dur, trunc};
use crate::view::unseen::mark_of;

/// `t`'s card under orchestrator `pane`; `seed` shifts its sweep. Its height.
pub fn thread_card(
    cv: &mut Screen,
    p: Place,
    t: &Thread,
    view: &View,
    seed: i64,
    pane: &str,
) -> i32 {
    let Place {
        x, y, w, is_sel, ..
    } = p;
    let hh = if is_sel && !t.phases.is_empty() {
        6 + t.phases.len() as i32
    } else {
        5
    };
    let card = Area { x, y, w, h: hh };
    let target = Target::Thread {
        pane: pane.to_owned(),
        thread: t.id.clone(),
    };
    cv.area(x, x + w, y, y + hh, target.clone());
    let bc = border(view, &target, p.focus, is_sel);
    rbox(cv, x, y, w, hh, bc);
    let agents: Vec<_> = t.agents.iter().collect();
    let st = state_of(&agents, t.is_done(), view.frame, &Extras::default(), None);
    edge(cv, card, &st, view.frame, seed);
    pulse_box(
        cv,
        card,
        mark_of(&t.agents, &view.unseen, true),
        view.frame,
        bc,
    );
    let mw = marks_right(cv, x + w - 2, y + 1, &marks_of(&agents, false, 0), None);
    let (g, gc) = thread_glyph(t, view);
    let name = Seg::new(trunc(&t.name, w - 7 - mw), if is_sel { HI } else { FG }).bold();
    cv.segs(
        x + 2,
        y + 1,
        &[Seg::new(g, gc).bold(), Seg::plain(" "), name],
    );
    cv.segs(
        x + 4,
        y + 2,
        &[Seg::new(trunc(&phase_sub(t, view), w - 6), DIM)],
    );
    let pods = Pods {
        max_n: ((w - 14) / 4).max(1) as usize,
        sel_agent: view.agent.as_deref().filter(|_| is_sel),
        orch: Some(pane),
        top_only: true,
    };
    pods_row(cv, x + 4, y + 3, t, view, pods);
    if is_sel {
        for i in 0..t.phases.len() {
            step_line(cv, x + 4, y + 5 + i as i32, t, i, w - 6, view);
        }
    }
    let age = t
        .started
        .filter(|s| *s != 0.0)
        .map_or_else(|| "queued".into(), |s| dur(Some(view.now - s)));
    tags(cv, x, y + hh - 1, w, &[Some(age), t.worktree.clone()]);
    hh
}

/// `o`'s background tasks (design turn 8): a card above its threads. Its height.
pub fn bg_card(cv: &mut Screen, p: Place, o: &Orchestrator, view: &View) -> i32 {
    let Place {
        x, y, w, is_sel, ..
    } = p;
    let n = o.bg.len();
    let hh = 3 + n as i32;
    let target = Target::Thread {
        pane: o.pane_id.clone(),
        thread: format!("{BG_CARD}{}", o.pane_id),
    };
    cv.area(x, x + w, y, y + hh, target.clone());
    rbox(cv, x, y, w, hh, border(view, &target, p.focus, is_sel));
    let waiting = OrchState {
        st: Status::Background,
        glyph: String::new(),
        label: String::new(),
        working: false,
        bg: 0,
    };
    edge(cv, Area { x, y, w, h: hh }, &waiting, view.frame, 0);
    let plural = if n == 1 { "" } else { "s" };
    let title = [
        Seg::new(
            clock(view.frame, 0).to_string(),
            status(&Status::Background),
        )
        .bold(),
        Seg::plain(" "),
        Seg::new("background", if is_sel { HI } else { FG }).bold(),
        Seg::new(
            trunc(&format!("  {n} task{plural} · main session"), w - 15),
            DIM,
        ),
    ];
    cv.segs(x + 2, y + 1, &title);
    for (i, b) in o.bg.iter().enumerate() {
        cv.segs(
            x + 4,
            y + 2 + i as i32,
            &bg_row(b, w - 7, view.now, view.frame, i),
        );
    }
    if let Some(last) = o.last_activity.filter(|l| *l != 0.0) {
        let ended = format!("turn ended {} ago", dur(Some(view.now - last)));
        tags(cv, x, y + hh - 1, w, &[Some(ended)]);
    }
    hh
}
