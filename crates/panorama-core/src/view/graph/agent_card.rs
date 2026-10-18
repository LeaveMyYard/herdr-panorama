//! An agent's card in its depth column (Python `render.agent_card`): three rows, or unfolded
//! with its details and action hints when it is the selected agent.

use super::column::Place;
use crate::model::{Agent, Status, Thread};
use crate::view::agent_text::{age_of, role_label, short_id};
use crate::view::cards::Area;
use crate::view::cards::actions::actions;
use crate::view::cards::boxes::{border, rbox};
use crate::view::cards::details::{agent_detail_lines, label};
use crate::view::cards::edge::edge;
use crate::view::cards::pulse::pulse_box;
use crate::view::cards::tags::tags;
use crate::view::colour::Rgb;
use crate::view::orch_state::OrchState;
use crate::view::palette::{DIM, FAINT, FG, HI, status};
use crate::view::pod::apod;
use crate::view::screen::Screen;
use crate::view::seg::{Seg, fit_segs};
use crate::view::state::View;
use crate::view::target::{AgentRef, Target};
use crate::view::text::{dur, trunc};
use crate::view::unseen::mark_of;

/// What an agent card shows besides the agent: its thread, whether the inbox answers, and
/// how it sits in the tree.
#[derive(Clone, Copy, Debug)]
pub struct AgentCtx<'a> {
    pub t: &'a Thread,
    pub inbox_ok: bool,
    /// On the selected agent's path (an agent that spawned it): drawn as selected.
    pub on_path: bool,
    /// Its children's column is off to the right: the card says `› n`.
    pub kids_off: usize,
}

/// `a`'s card at `p` (`p.focus`: the keyboard is in its column). Its height.
pub fn agent_card(cv: &mut Screen, p: Place, a: &Agent, c: AgentCtx, view: &View) -> i32 {
    let r = AgentRef {
        pane: view.orch.clone().unwrap_or_default(),
        thread: c.t.id.clone(),
        agent: a.id.clone(),
    };
    if !p.is_sel {
        return folded(cv, p, a, c, view, Target::Agent(r));
    }
    let Place { x, y, w, .. } = p;
    let lines = agent_detail_lines(a, w - 6, view.now, view.frame);
    let hh = 6 + lines.len() as i32;
    frame(
        cv,
        Area { x, y, w, h: hh },
        a,
        view,
        (Target::Agent(r.clone()), p.focus, true),
    );
    let st = state_words(a, view.now);
    let room = (x + w - 2 - st.chars().count() as i32 - 1 - (x + 6)).max(1); // never into the state
    cv.segs(x + 6, y + 1, &fit_segs(title(a, HI), room));
    let st_x = x + w - 2 - st.chars().count() as i32;
    cv.segs(st_x, y + 1, &[Seg::new(st, status(&a.status)).bold()]);
    for (i, line) in lines.iter().enumerate() {
        cv.segs(x + 4, y + 3 + i as i32, line);
    }
    actions(cv, x + 4, y + hh - 2, a, c.inbox_ok, Some(&r), Some(w - 6));
    let phase = a.phases.last().and_then(|&i| phase_at(c.t, i));
    tags(
        cv,
        x,
        y + hh - 1,
        w,
        &[age_of(a, view.now), calls(a), phase],
    );
    hh
}

/// An unselected agent: its pod, title and activity on one line, its age and calls below.
fn folded(cv: &mut Screen, p: Place, a: &Agent, c: AgentCtx, view: &View, target: Target) -> i32 {
    let Place { x, y, w, .. } = p;
    frame(
        cv,
        Area { x, y, w, h: 3 },
        a,
        view,
        (target, false, c.on_path),
    );
    let right = if c.kids_off > 0 {
        format!("› {}", c.kids_off)
    } else {
        String::new()
    };
    let rw = if right.is_empty() {
        0
    } else {
        right.chars().count() as i32 + 1
    };
    let fg = if c.on_path { HI } else { FG };
    let x2 = cv.segs(
        x + 6,
        y + 1,
        &fit_segs(title(a, fg), (x + w - 2 - rw - (x + 6)).max(1)),
    );
    let room = (x + w - 3 - x2 - 3 - rw).max(0);
    let doing = if a.activity.is_empty() {
        label(&a.status)
    } else {
        &a.activity
    };
    cv.segs(x2 + 3, y + 1, &[Seg::new(trunc(doing, room), FAINT)]);
    if !right.is_empty() {
        let rx = x + w - 2 - right.chars().count() as i32;
        cv.segs(rx, y + 1, &[Seg::new(right, DIM).bold()]);
    }
    tags(cv, x, y + 2, w, &[age_of(a, view.now), calls(a)]);
    3
}

/// The card's area, box, edge, pulse and pod; `look`: its target, focus and selection.
fn frame(cv: &mut Screen, card: Area, a: &Agent, view: &View, look: (Target, bool, bool)) {
    let (target, focus, is_sel) = look;
    let Area { x, y, w, h } = card;
    cv.area(x, x + w, y, y + h, target.clone());
    let bc = border(view, &target, focus, is_sel);
    rbox(cv, x, y, w, h, bc);
    let st = OrchState {
        st: a.status.clone(),
        glyph: String::new(),
        label: String::new(),
        working: a.status == Status::Running,
        bg: 0,
    };
    edge(cv, card, &st, view.frame, a.seq);
    pulse_box(cv, card, mark_of([a], &view.unseen, false), view.frame, bc);
    apod(cv, x + 2, y + 1, a, view.frame, false);
}

/// Role, short id and model.
fn title(a: &Agent, fg: Rgb) -> Vec<Seg> {
    let model = a
        .model
        .as_deref()
        .filter(|m| !m.is_empty())
        .map_or(String::new(), |m| format!(" · {m}"));
    vec![
        Seg::new(role_label(a), fg).bold(),
        Seg::new(format!(" {}", short_id(a)), FAINT),
        Seg::new(model, DIM),
    ]
}

/// The selected card's state on the right: its label, how long ago it ended, or how long it
/// has waited on its oldest background task.
fn state_words(a: &Agent, now: f64) -> String {
    let mut st = a.status.label().to_owned();
    if let Some(ended) = a.ended.filter(|e| *e != 0.0 && !a.status.is_live()) {
        st += &format!(" {} ago", dur(Some(now - ended)));
    }
    let oldest =
        a.bg.iter()
            .filter_map(|b| b.started.filter(|s| *s != 0.0))
            .reduce(f64::min);
    if let (Status::Background, Some(oldest)) = (&a.status, oldest) {
        st += &format!(" {}", dur(Some(now - oldest)));
    }
    st
}

/// `n calls`, none at 0.
fn calls(a: &Agent) -> Option<String> {
    (a.tool_calls != 0).then(|| format!("{} calls", a.tool_calls))
}

/// The name of `t`'s phase `i` (Python's list index: a negative one counts from the end).
fn phase_at(t: &Thread, i: i64) -> Option<String> {
    let n = t.phases.len() as i64;
    let i = if i < 0 { i + n } else { i };
    (0..n)
        .contains(&i)
        .then(|| t.phases[i as usize].name.clone())
}
