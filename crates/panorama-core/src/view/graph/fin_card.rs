//! A group's "n finished" card (Python `render.fin_card`): up to five pods of the agents it
//! folds, a count by role, and whether it is unfolded.

use super::thread_card::Place;
use crate::model::{Agent, Thread};
use crate::view::agent_text::role_label;
use crate::view::cards::Area;
use crate::view::cards::boxes::{border, rbox};
use crate::view::cards::pulse::pulse_box;
use crate::view::nav::TreeEntry;
use crate::view::palette::FAINT;
use crate::view::pod::apod;
use crate::view::screen::Screen;
use crate::view::seg::Seg;
use crate::view::state::View;
use crate::view::target::Target;
use crate::view::text::trunc;
use crate::view::unseen::mark_of;

/// The card of entry `e` (a "n finished" entry of thread `t`) at `p`.
pub fn fin_card(cv: &mut Screen, p: Place, e: &TreeEntry, t: &Thread, view: &View) {
    let Place { x, y, w, .. } = p;
    let fin: Vec<&Agent> = e
        .fin
        .iter()
        .filter_map(|id| t.agents.iter().find(|a| &a.id == id))
        .collect();
    // the thread's own group keeps its old identity
    let target = Target::Fin {
        pane: view.orch.clone().unwrap_or_default(),
        thread: t.id.clone(),
        group: (e.key != t.id).then(|| e.key.clone()),
    };
    cv.area(x, x + w, y, y + 3, target.clone());
    let bc = border(view, &target, p.focus, p.is_sel);
    rbox(cv, x, y, w, 3, bc);
    let card = Area { x, y, w, h: 3 };
    pulse_box(
        cv,
        card,
        mark_of(fin.iter().copied(), &view.unseen, false),
        view.frame,
        bc,
    );
    let mut xx = x + 2;
    for a in fin.iter().take(5) {
        apod(cv, xx, y + 1, a, view.frame, false);
        xx += 4;
    }
    let mut roles: Vec<(&str, usize)> = vec![];
    for a in &fin {
        match roles.iter_mut().find(|(r, _)| *r == role_label(a)) {
            Some((_, n)) => *n += 1,
            None => roles.push((role_label(a), 1)),
        }
    }
    let by_role: Vec<String> = roles
        .iter()
        .map(|(r, n)| {
            if *n > 1 {
                format!("{r} ×{n}")
            } else {
                (*r).to_owned()
            }
        })
        .collect();
    let summary = format!("{} finished · {}", fin.len(), by_role.join(", "));
    cv.segs(
        xx + 1,
        y + 1,
        &[Seg::new(trunc(&summary, x + w - xx - 5), FAINT)],
    );
    let fold = if view.fin_open.contains(&e.key) {
        "▾"
    } else {
        "▸"
    };
    cv.put(x + w - 3, y + 1, fold, FAINT, None, false);
}
