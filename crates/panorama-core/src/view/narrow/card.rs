//! The narrow layout's cards under the selected root (plan `uniform-agent-tree.md` §4): one
//! per direct subagent, the same card at any depth (its glyph, title and the marks of its
//! subtree; a pod per descendant; its model and age). The card holding the selected agent is
//! opened: that agent's details and actions. The root's finished subagents fold into one
//! `✓ N finished · names` line.

use crate::model::{Agent, Orchestrator, Status, Swarm};
use crate::view::agent_text::{age_of, name, role_label, short_id};
use crate::view::anim::glyph;
use crate::view::cards::Area;
use crate::view::cards::actions::actions;
use crate::view::cards::boxes::{border, rbox};
use crate::view::cards::details::agent_detail_lines;
use crate::view::cards::edge::edge;
use crate::view::cards::marks::{marks_of, marks_right};
use crate::view::cards::pods::{Pods, pods_row};
use crate::view::nav::find_agent;
use crate::view::orch_state::{Extras, state_of};
use crate::view::palette::{DIM, FAINT, FG, HI, status};
use crate::view::pod::apod;
use crate::view::screen::Screen;
use crate::view::seg::Seg;
use crate::view::state::View;
use crate::view::target::{AgentRef, Target};
use crate::view::text::trunc;

/// What every card under one root shares.
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

/// The root's finished subagents folded into one line, `✓ N finished · names`, and
/// `+N older` for those the done window dropped. `ident`: its id in the depth-1 column; `None`
/// when the window dropped them all, and the line only counts them.
pub fn fin_card(
    cv: &mut Screen,
    y: i32,
    c: &CardCtx,
    fin: &[&Agent],
    ident: Option<&str>,
) -> Placed {
    if ident.is_some() {
        let target = Target::Fin {
            pane: c.o.pane_id.clone(),
            group: None,
        };
        cv.area(3, c.w - 1, y, y + 1, target);
    }
    let names: Vec<String> = fin.iter().map(|a| name(a)).collect();
    let mut parts = vec![];
    if c.o.hidden_finished > 0 {
        parts.push(format!("+{} older", c.o.hidden_finished));
    }
    if !names.is_empty() {
        parts.push(format!("{} finished · {}", names.len(), names.join(", ")));
    }
    let text = trunc(&format!(" {}", parts.join(" · ")), c.w - 9);
    cv.segs(
        5,
        y,
        &[
            Seg::new("✓", status(&Status::Done)).bold(),
            Seg::new(text, FAINT),
        ],
    );
    let mut sel = None;
    if ident.is_some() && c.view.agent.as_deref() == ident {
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

/// Agent `a`'s card at row `y`, `below` its descendants; `seed` shifts its edge's sweep;
/// `open`: it holds the selected agent, whose details it then shows.
pub fn agent_card(
    cv: &mut Screen,
    y: i32,
    c: &CardCtx,
    a: &Agent,
    below: &[&Agent],
    (seed, open): (i64, bool),
) -> Placed {
    let view = c.view;
    let picked = find_agent(Some(c.o), view.agent.as_deref()).filter(|_| open);
    let det = picked.map_or_else(Vec::new, |p| {
        agent_detail_lines(p, c.w - 10, view.now, view.frame)
    });
    let height = if picked.is_some() {
        det.len() as i32 + 8
    } else {
        4
    };
    let card = Area {
        x: 3,
        y,
        w: c.w - 4,
        h: height,
    };
    let target = Target::Agent(AgentRef {
        pane: c.o.pane_id.clone(),
        agent: a.id.clone(),
    });
    cv.area(3, c.w - 1, y, y + height, target.clone());
    rbox(
        cv,
        card.x,
        y,
        card.w,
        height,
        border(view, &target, open && view.focus >= 1, open),
    );
    let mut all: Vec<&Agent> = vec![a];
    all.extend(below);
    let state = state_of(
        &all,
        !all.iter().any(|a| a.status.is_live()),
        view.frame,
        &Extras::default(),
        None,
    );
    edge(cv, card, &state, view.frame, seed);
    head(cv, y, c, a, &all, below, open);
    let mut sel = None;
    if open {
        if let Some(p) = picked {
            agent_block(cv, y + 4, c, (p, p.id == a.id), &det);
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

/// The card's two lines: glyph, title and its subtree's marks; a pod per descendant, then
/// its model and age.
fn head(
    cv: &mut Screen,
    y: i32,
    c: &CardCtx,
    a: &Agent,
    all: &[&Agent],
    below: &[&Agent],
    open: bool,
) {
    let (view, w) = (c.view, c.w);
    let mw = marks_right(cv, w - 3, y + 1, &marks_of(all, false, 0), None);
    let g = Seg::new(glyph(a, view.frame), status(&a.status)).bold();
    let title = Seg::new(trunc(&name(a), w - 13 - mw), if open { HI } else { FG }).bold();
    cv.segs(5, y + 1, &[g, Seg::plain(" "), title]);
    let sel_agent = if open { view.agent.as_deref() } else { None };
    let pods = Pods {
        max_n: 6,
        sel_agent,
        orch: Some(&c.o.pane_id),
    };
    pods_row(cv, 7, y + 2, below, view, pods);
    let model = a.model.as_deref().filter(|m| !m.is_empty());
    let age = age_of(a, view.now);
    let sub: Vec<&str> = [model, age.as_deref()].into_iter().flatten().collect();
    let sub = trunc(&sub.join(" · "), w - 40);
    cv.segs(
        w - 3 - sub.chars().count() as i32,
        y + 2,
        &[Seg::new(sub, DIM)],
    );
}

/// The selected agent at row `r`: its pod, title (not when it is the card's `own` agent,
/// whose head already shows it), then `role id · model · age · calls`, its detail lines
/// `det`, and its action hints.
fn agent_block(
    cv: &mut Screen,
    mut r: i32,
    c: &CardCtx,
    (a, own): (&Agent, bool),
    det: &[Vec<Seg>],
) {
    let (view, w) = (c.view, c.w);
    let ident = AgentRef {
        pane: c.o.pane_id.clone(),
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
    // titled: the title, then `role id · model` with the rest; else the role, then the id
    let (role, lead) = if a.title.is_empty() || own {
        (role_label(a).to_owned(), String::new())
    } else {
        (
            // at most 3/5 of the row: its role, id, age and calls keep room
            trunc(&a.title, ((w - 16) * 3 / 5).max(8)),
            format!(" {}", role_label(a)),
        )
    };
    let about = format!(
        "{lead} {}{model} · {age} · {} calls",
        short_id(a),
        a.tool_calls
    );
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
