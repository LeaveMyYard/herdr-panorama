//! A row of a thread's agent pods (Python `render.pods_row`): live and failed agents first
//! (most urgent first), finished ones while there is room, then `+N done` for the rest; with
//! `top_only`, the thread's own agents and `+N nested` for those they spawned.

use crate::model::{Agent, Status, Thread, TreeKids};
use crate::view::nav::path_ids;
use crate::view::palette::FAINT;
use crate::view::pod::apod;
use crate::view::screen::Screen;
use crate::view::seg::Seg;
use crate::view::state::View;
use crate::view::target::{AgentRef, Target};

/// How a row of pods is drawn.
#[derive(Clone, Copy, Debug, Default)]
pub struct Pods<'a> {
    /// At most this many pods.
    pub max_n: usize,
    /// The agent whose pod is bracketed.
    pub sel_agent: Option<&'a str>,
    /// The orchestrator's pane: each pod is then a click target selecting its agent.
    pub orch: Option<&'a str>,
    /// Only the thread's own agents, then `+N nested` for the agents they spawned; the pod
    /// bracketed is the one on `sel_agent`'s path.
    pub top_only: bool,
}

/// `t`'s pods from `(x, y)`.
pub fn pods_row(s: &mut Screen, x: i32, y: i32, t: &Thread, view: &View, p: Pods) {
    let Pods {
        max_n,
        sel_agent,
        orch,
        top_only,
    } = p;
    let (mut ags, nested, path): (Vec<&Agent>, usize, Vec<String>) = if top_only {
        let own = TreeKids::of(t).children(None).to_vec();
        let nested = t.agents.len() - own.len();
        (own, nested, path_ids(Some(t), sel_agent))
    } else {
        (t.agents.iter().collect(), 0, vec![])
    };
    let sel_agent = path.first().map(String::as_str).or(sel_agent);
    let start = |a: &&Agent| a.started.unwrap_or(0.0);
    ags.sort_by(|a, b| {
        (a.status.urgency(), start(a))
            .partial_cmp(&(b.status.urgency(), start(b)))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let (lv, fin): (Vec<&Agent>, Vec<&Agent>) = ags
        .into_iter()
        .partition(|a| a.status.is_live() || a.status == Status::Failed);
    let room = max_n.saturating_sub(lv.len());
    let show_f = if fin.len() <= room {
        &fin[..]
    } else {
        &fin[..room.saturating_sub(1)]
    };
    let shown: Vec<&Agent> = lv.iter().chain(show_f).take(max_n).copied().collect();
    let mut x = x;
    for a in &shown {
        apod(s, x, y, a, view.frame, sel_agent == Some(a.id.as_str()));
        if let Some(o) = orch {
            let r = AgentRef {
                pane: o.to_owned(),
                thread: t.id.clone(),
                agent: a.id.clone(),
            };
            s.target(x, x + 3, y, Target::Agent(r));
        }
        x += 4;
    }
    let hidden = lv.len() + fin.len() - shown.len();
    if hidden > 0 {
        let text = if lv.len() <= max_n {
            format!("+{hidden} done")
        } else {
            format!("+{hidden}")
        };
        x = s.segs(x, y, &[Seg::new(text, FAINT)]) + 1;
    }
    if nested > 0 {
        s.segs(x, y, &[Seg::new(format!("+{nested} nested"), FAINT)]);
    }
}
