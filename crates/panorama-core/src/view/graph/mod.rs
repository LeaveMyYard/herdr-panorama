//! The wide graph, design 2a (Python `render.render` from 120 columns) with depth-only
//! columns (plan `uniform-agent-tree.md` §4): the header, the columns by depth (the roots
//! are depth 0) with the links between them, the legend of tiles and the wide key hints.
//!
//! Every depth draws the same [`card`]: the roots' column ([`orchs`], [`root_card`]) on its
//! own tall canvas, the selected root's agent tree ([`place`]d in its depth columns,
//! [`agent_card`]) on one canvas. [`layout`] decides which columns fit around the focus.
//!
//! Every colour tick renders the page in full (Python restyled the animated cells instead;
//! its restyle tests are retired on that ground, and come back if a later optimisation
//! restyles here).

pub mod agent_card;
mod agents;
pub mod card;
pub mod column;
mod fin_card;
pub mod layout;
pub mod orchs;
pub mod place;
pub mod root_card;
mod sideways;
mod wide;

use super::chrome;
use super::palette::BG;
use super::screen::Screen;
use super::state::View;
use crate::model::Swarm;

/// The wide page, `w` × `h`; `stale`: why the model may be out of date.
pub fn page(swarm: &Swarm, view: &View, w: i32, h: i32, stale: Option<&str>) -> Screen {
    let mut s = Screen::new(w, h);
    s.fill_bg(0, 0, w, h, BG);
    chrome::header(&mut s, swarm, view, stale);
    let sideways = wide::columns(&mut s, swarm, view);
    chrome::legend(&mut s, h - 3, true);
    chrome::keys(&mut s, true, sideways, view.zoom_hint, swarm.inbox_ok);
    s
}

#[cfg(test)]
mod tests;
