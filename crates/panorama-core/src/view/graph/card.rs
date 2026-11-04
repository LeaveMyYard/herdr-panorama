//! The graph's one card (plan `uniform-agent-tree.md` §4), at every depth: a root is just the
//! depth-0 card. Its frame (area, box, animated edge, unseen pulse and pod), its first line
//! (title, the subtree's need marks, its own state or what it does) and its tags on the
//! bottom border. What a root and an agent put in it is theirs ([`super::root_card`],
//! [`super::agent_card`]).

use crate::model::{Agent, Status, TreeKids};
use crate::view::cards::Area;
use crate::view::cards::boxes::{border, rbox};
use crate::view::cards::edge::edge;
use crate::view::cards::marks::{marks_of, marks_right};
use crate::view::cards::pulse::pulse_box;
use crate::view::orch_state::OrchState;
use crate::view::screen::Screen;
use crate::view::seg::{Seg, fit_segs, width};
use crate::view::state::View;
use crate::view::target::Target;
use crate::view::unseen::Mark;

/// How a card looks: what a click on it hits, whether the keyboard is on it, whether it is
/// drawn as selected, its state (edge and pod), the edge's sweep seed and its unseen pulse.
pub struct Look<'a> {
    pub target: Target,
    pub focus: bool,
    pub is_sel: bool,
    pub st: &'a OrchState,
    pub seed: i64,
    pub pulse: Option<Mark>,
}

/// The frame: area, box (hover, focus and selection from `view`), edge and pulse. The pod
/// is the caller's (an agent's glyph animates on its own).
pub fn draw_frame(cv: &mut Screen, card: Area, look: &Look, view: &View) {
    let Area { x, y, w, h } = card;
    cv.area(x, x + w, y, y + h, look.target.clone());
    let bc = border(view, &look.target, look.focus, look.is_sel);
    rbox(cv, x, y, w, h, bc);
    edge(cv, card, look.st, view.frame, look.seed);
    pulse_box(cv, card, look.pulse, view.frame, bc);
}

/// The first line from `x + 6` to the right border: `title`, then `doing` (faint) in the
/// room left, with `right` (marks, the state, `› n`) right-aligned; the marks' `?N`
/// answers `orch` when given.
pub struct Line<'a> {
    pub title: Vec<Seg>,
    pub doing: Option<Seg>,
    pub marks: Vec<Seg>,
    pub orch: Option<&'a str>,
    pub right: Vec<Seg>,
}

/// Draws `l` on row `y` of the `w`-wide card at `x`.
pub fn line(cv: &mut Screen, x: i32, y: i32, w: i32, l: Line) {
    let rw = width(&l.right);
    let mut xr = x + w - 2;
    if rw > 0 {
        cv.segs(xr - rw, y, &l.right);
        xr -= rw + 1;
    }
    let mw = marks_right(cv, xr, y, &l.marks, l.orch);
    if mw > 0 {
        xr -= mw + 1;
    }
    let x2 = cv.segs(x + 6, y, &fit_segs(l.title, (xr - (x + 6)).max(1)));
    if let Some(d) = l.doing {
        let room = xr - x2 - 3;
        if room > 0 {
            cv.segs(x2 + 3, y, &fit_segs(vec![d], room));
        }
    }
}

/// The need marks (✗ ! ?N) of `a` and every agent below it, but the one its own pod
/// already says.
pub fn subtree_marks(a: &Agent, kids: &TreeKids) -> Vec<Seg> {
    let mut all = vec![a];
    let mut i = 0;
    while i < all.len() && all.len() < 4096 {
        all.extend(kids.children(Some(&all[i].id)).iter().copied());
        i += 1;
    }
    let mut mk = marks_of(&all, false, 0);
    own_out(&mut mk, &a.status);
    mk
}

/// Drops the mark the pod already says (a failed card's `✗`, a blocked one's `!`).
pub fn own_out(mk: &mut Vec<Seg>, st: &Status) {
    mk.retain(|m| {
        !(m.text == "✗" && *st == Status::Failed || m.text == "!" && *st == Status::Blocked)
    });
}
