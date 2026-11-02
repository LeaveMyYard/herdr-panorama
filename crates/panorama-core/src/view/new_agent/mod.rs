//! The new-agent modal, design 1C "Coming soon" (Claude Design `herdr-tui.js` `modalCSoon`):
//! the harness carousel (the selected art in its colour, the neighbours ghosted, ‹ name ›,
//! position dots), the name with its agent name under it, the working directory, a status
//! row and the key hints. A harness panorama cannot start yet is drawn grey with "Coming
//! soon", and the start hint is greyed out.
//!
//! Rendering only: what the keys and clicks do, and starting the agent, are the form's.

pub mod field;
pub mod harness;
pub mod look;

use self::field::{Input, ROWS};
use self::look::{
    ACCENT, Border, DIM, FAINT, GHOST_ART, GHOST_TAG, MUTED, PANEL, TEXT, WARN, art, boxed, len,
    put_upto,
};
use crate::view::Screen;
use crate::view::seg::Seg;
pub use harness::{HARNESSES, Harness, step};

/// The modal's width and height.
pub const W: i32 = 76;
pub const H: i32 = 27;

const TITLE: &str = " + New agent ";
const SOON: &str = "Coming soon";
const START: &str = "enter on the last field or ctrl+s: start";
const HINT: &str = "enter on the last field or ctrl+s: start · tab: next field · esc: cancel";
const NAME_LABEL: &str = "Name (also the tab label)";
const CWD_LABEL: &str = "Working directory";

/// The field with the focus.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Field {
    Name,
    Cwd,
}

/// What the modal shows.
#[derive(Clone, Copy, Debug)]
pub struct Form<'a> {
    /// The selected harness, an index into [`HARNESSES`].
    pub harness: usize,
    pub name: &'a str,
    /// The agent name the name makes (empty: no line under the name).
    pub slug: &'a str,
    pub cwd: &'a str,
    /// The focused field (`None`: none, as while the agent starts) and its cursor.
    pub focus: Option<(Field, usize)>,
    /// The status row's text (`None`: empty).
    pub status: Option<&'a str>,
    /// The carousel has the focus (←/→ switch the harness): its name is highlighted in the
    /// focused field's accent. Not in the design: the operator's addition (Q213).
    pub carousel: bool,
}

/// A click target that steps the harness by `d`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Step {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub d: i32,
}

/// The rendered modal and its click targets.
pub struct Modal {
    pub screen: Screen,
    /// In the design's order: the first one under a cell wins.
    pub steps: Vec<Step>,
}

impl Modal {
    /// The step a click at `(x, y)` makes.
    pub fn step_at(&self, x: i32, y: i32) -> Option<i32> {
        let inside = |s: &&Step| (s.x..s.x + s.w).contains(&x) && (s.y..s.y + s.h).contains(&y);
        self.steps.iter().find(inside).map(|s| s.d)
    }
}

/// The modal centred on a `w` × `h` screen (at its top left when the screen is smaller).
pub fn render(form: &Form<'_>, w: i32, h: i32) -> Modal {
    let mut s = Screen::new(w, h);
    let (mx, my) = ((w - W).max(0) / 2, (h - H).max(0) / 2);
    boxed(
        &mut s,
        (mx, my, W, H),
        look::BORDER,
        Some(PANEL),
        Border::Round,
        None,
    );
    s.put(mx + 2, my, TITLE, TEXT, Some(PANEL), true);
    let steps = carousel(
        &mut s,
        form.harness % HARNESSES.len(),
        form.carousel,
        mx,
        my,
    );
    let off = !HARNESSES[form.harness % HARNESSES.len()].supported();
    let focus = |f: Field| form.focus.filter(|(g, _)| *g == f).map(|(_, c)| c);
    let y = my + 2 + 8 + 4;
    let sub = [("agent name: ", DIM), (form.slug, MUTED)];
    let name = Input {
        label: NAME_LABEL,
        value: form.name,
        cursor: focus(Field::Name),
        sub: if form.slug.is_empty() { &[] } else { &sub },
    };
    field::draw(&mut s, mx + 3, y, W - 6, &name);
    let cwd = Input {
        label: CWD_LABEL,
        value: form.cwd,
        cursor: focus(Field::Cwd),
        sub: &[],
    };
    field::draw(&mut s, mx + 3, y + ROWS + 1, W - 6, &cwd);
    if let Some(status) = form.status {
        put_upto(
            &mut s,
            mx + 2,
            my + H - 3,
            mx + W - 2,
            Seg::new(status, WARN).on(PANEL),
        );
    }
    let x1 = s.put(
        mx + 2,
        my + H - 2,
        START,
        if off { FAINT } else { DIM },
        Some(PANEL),
        false,
    );
    s.put(
        x1,
        my + H - 2,
        &HINT[START.len()..],
        DIM,
        Some(PANEL),
        false,
    );
    Modal { screen: s, steps }
}

/// The carousel under the title, down to its rule; its click targets.
fn carousel(s: &mut Screen, sel: usize, focused: bool, mx: i32, my: i32) -> Vec<Step> {
    let (h, prev, next) = (
        &HARNESSES[sel],
        &HARNESSES[step(sel, -1)],
        &HARNESSES[step(sel, 1)],
    );
    let off = !h.supported();
    let (cx, ay) = (mx + W / 2, my + 2);
    art(s, mx + 4, ay + 1, &prev.big, GHOST_ART, false);
    art(s, mx + W - 17, ay + 1, &next.big, GHOST_ART, false);
    if !prev.supported() {
        s.put(
            mx + 10 - len(SOON) / 2,
            ay + 8,
            SOON,
            GHOST_TAG,
            Some(PANEL),
            false,
        );
    }
    if !next.supported() {
        s.put(
            mx + W - 11 - len(SOON) / 2,
            ay + 8,
            SOON,
            GHOST_TAG,
            Some(PANEL),
            false,
        );
    }
    art(s, cx - 6, ay, &h.big, if off { DIM } else { h.col }, !off);
    let ry = ay + 8;
    s.put(cx - 18, ry, "‹", ACCENT, Some(PANEL), true);
    s.put(cx + 18, ry, "›", ACCENT, Some(PANEL), true);
    s.put(
        cx - len(h.name) / 2,
        ry,
        h.name,
        match (focused, off) {
            (true, _) => ACCENT,
            (false, true) => MUTED,
            (false, false) => TEXT,
        },
        Some(PANEL),
        focused || !off,
    );
    let dots: Vec<&str> = (0..HARNESSES.len())
        .map(|i| if i == sel { "●" } else { "·" })
        .collect();
    let dots = dots.join(" ");
    let dx = cx - len(&dots) / 2;
    s.put(dx, ry + 1, &dots, DIM, Some(PANEL), false);
    s.put(
        dx + sel as i32 * 2,
        ry + 1,
        "●",
        if off { MUTED } else { h.col },
        Some(PANEL),
        false,
    );
    if off {
        s.put(cx - len(SOON) / 2, ry + 2, SOON, ACCENT, Some(PANEL), false);
    }
    s.put(
        mx + 1,
        ry + 3,
        &"─".repeat((W - 2) as usize),
        FAINT,
        Some(PANEL),
        false,
    );
    let step = |x, y, w, h, d| Step { x, y, w, h, d };
    vec![
        step(mx + 1, ay, 18, 9, -1),
        step(mx + W - 19, ay, 18, 9, 1),
        step(cx - 20, ry, 4, 1, -1),
        step(cx + 17, ry, 4, 1, 1),
    ]
}

#[cfg(test)]
mod tests;
