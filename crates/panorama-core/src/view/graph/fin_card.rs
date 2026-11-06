//! A group's "n finished" card (Python `render.fin_card`): up to five pods of the agents it
//! folds, their names (`×n` for a repeated one), the root's own group also counting the
//! finished agents the window hid (`+n older`), and whether it is unfolded.

use super::column::Place;
use crate::model::{Agent, Root};
use crate::view::agent_text::name;
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

/// The card of entry `e` (a "n finished" entry of root `o`) at `p`.
pub fn fin_card(cv: &mut Screen, p: Place, e: &TreeEntry, o: &Root, view: &View) {
    let Place { x, y, w, .. } = p;
    let fin: Vec<&Agent> = e
        .fin
        .iter()
        .filter_map(|id| o.agents.iter().find(|a| &a.id == id))
        .collect();
    let own = e.key == o.pane_id;
    let target = Target::Fin {
        pane: o.pane_id.clone(),
        group: (!own).then(|| e.key.clone()),
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
    let mut names: Vec<(String, usize)> = vec![];
    for a in &fin {
        let nm = name(a);
        match names.iter_mut().find(|(r, _)| *r == nm) {
            Some((_, n)) => *n += 1,
            None => names.push((nm, 1)),
        }
    }
    let names: Vec<String> = names
        .into_iter()
        .map(|(r, n)| if n > 1 { format!("{r} ×{n}") } else { r })
        .collect();
    let older = if own && o.hidden_finished > 0 {
        format!(" · +{} older", o.hidden_finished)
    } else {
        String::new()
    };
    let summary = format!("{} finished{older} · {}", fin.len(), names.join(", "));
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
