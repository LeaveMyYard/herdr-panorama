//! An agent's card in its depth column (Python `render.agent_card`): three rows, or unfolded
//! with its details and action hints when it is the selected agent.

use super::card::{Line, Look, draw_frame, line, subtree_marks};
use super::column::Place;
use crate::model::{Agent, Root, Status, TreeKids};
use crate::view::agent_text::{about, age_of, role_label, short_id};
use crate::view::cards::Area;
use crate::view::cards::actions::actions;
use crate::view::cards::details::{agent_detail_lines, label};
use crate::view::cards::tags::tags;
use crate::view::colour::Rgb;
use crate::view::orch_state::OrchState;
use crate::view::palette::{DIM, FAINT, FG, HI, status};
use crate::view::pod::apod;
use crate::view::screen::Screen;
use crate::view::seg::Seg;
use crate::view::state::View;
use crate::view::target::{AgentRef, Target};
use crate::view::text::{dur, trunc};
use crate::view::unseen::mark_of;

/// What an agent card shows besides the agent: its root and the root's tree (for the
/// subtree's marks), whether the inbox answers, and how it sits in the tree.
#[derive(Clone, Copy)]
pub struct AgentCtx<'a> {
    pub o: &'a Root,
    pub kids: &'a TreeKids<'a>,
    pub inbox_ok: bool,
    /// On the selected agent's path (an agent that spawned it): drawn as selected.
    pub on_path: bool,
    /// Its children's column is off to the right: the card says `› n`.
    pub kids_off: usize,
}

/// `a`'s card at `p` (`p.focus`: the keyboard is in its column). Its height.
pub fn agent_card(cv: &mut Screen, p: Place, a: &Agent, c: AgentCtx, view: &View) -> i32 {
    let r = AgentRef {
        pane: c.o.pane_id.clone(),
        agent: a.id.clone(),
    };
    if !p.is_sel {
        return folded(cv, p, a, c, view, Target::Agent(r));
    }
    let Place { x, y, w, .. } = p;
    let lines = detail_lines(a, w - 6, view);
    let hh = 6 + lines.len() as i32;
    frame(
        cv,
        Area { x, y, w, h: hh },
        a,
        view,
        (Target::Agent(r.clone()), p.focus, true),
    );
    let st = state_words(a, view.now);
    let l = Line {
        title: title(a, HI),
        doing: None,
        marks: subtree_marks(a, c.kids),
        orch: None,
        right: vec![Seg::new(st, status(&a.status)).bold()],
    };
    line(cv, x, y + 1, w, l);
    for (i, line) in lines.iter().enumerate() {
        cv.segs(x + 4, y + 3 + i as i32, line);
    }
    actions(cv, x + 4, y + hh - 2, a, c.inbox_ok, Some(&r), Some(w - 6));
    tags(cv, x, y + hh - 1, w, &[age_of(a, view.now), calls(a)]);
    hh
}

/// The unfolded card's detail lines in `w` cells: a titled agent's `role id · model` first,
/// then [`agent_detail_lines`]. The layout sizes the card from these too.
pub fn detail_lines(a: &Agent, w: i32, view: &View) -> Vec<Vec<Seg>> {
    let mut lines = agent_detail_lines(a, w, view.now, view.frame);
    if let Some(about) = about(a) {
        lines.insert(0, vec![Seg::new(trunc(&about, w), DIM)]);
    }
    lines
}

/// An unselected agent: its pod, title, activity and the subtree's marks on one line, its
/// model, age and calls below.
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
        vec![Seg::new(format!("› {}", c.kids_off), DIM).bold()]
    } else {
        vec![]
    };
    let doing = if a.activity.is_empty() {
        label(&a.status)
    } else {
        &a.activity
    };
    let l = Line {
        title: title(a, if c.on_path { HI } else { FG }),
        doing: Some(Seg::new(doing, FAINT)),
        marks: subtree_marks(a, c.kids),
        orch: None,
        right,
    };
    line(cv, x, y + 1, w, l);
    // titled: the model joins the tags (the role and id are in the unfolded card)
    let model = a.model.clone().filter(|_| !a.title.is_empty());
    tags(cv, x, y + 2, w, &[model, age_of(a, view.now), calls(a)]);
    3
}

/// The card's frame and pod; `look`: its target, focus and selection.
fn frame(cv: &mut Screen, card: Area, a: &Agent, view: &View, look: (Target, bool, bool)) {
    let (target, focus, is_sel) = look;
    let st = OrchState {
        st: a.status.clone(),
        glyph: String::new(),
        label: String::new(),
        working: a.status == Status::Running,
        bg: 0,
    };
    let look = Look {
        target,
        focus,
        is_sel,
        st: &st,
        seed: a.seq,
        pulse: mark_of([a], &view.unseen, false),
    };
    draw_frame(cv, card, &look, view);
    apod(cv, card.x + 2, card.y + 1, a, view.frame, false);
}

/// Its title; untitled (an older producer): role, short id and model.
fn title(a: &Agent, fg: Rgb) -> Vec<Seg> {
    if !a.title.is_empty() {
        return vec![Seg::new(a.title.clone(), fg).bold()];
    }
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
