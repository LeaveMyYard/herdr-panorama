//! The graph's header line: `herdr panorama`, the filters (click targets), the
//! refresh-failed badge, and the totals and the clock on the right.

use crate::model::{Status, Swarm};
use crate::view::filter::Filter;
use crate::view::palette::{DIM, FAINT, HDR, HI, ON_LIGHT, status};
use crate::view::screen::Screen;
use crate::view::seg::{Seg, width};
use crate::view::state::View;
use crate::view::target::Target;
use crate::view::text::{hhmm, trunc};

/// Line 0: `herdr panorama`, the filters (click targets), the refresh-failed badge, and the
/// totals and the clock on the right (only the clock below 60 columns).
pub fn header(s: &mut Screen, swarm: &Swarm, view: &View, stale: Option<&str>) {
    s.fill_bg(0, 0, s.w(), 1, HDR);
    let mut x = s.segs(
        1,
        0,
        &[Seg::new("herdr", HI).bold(), Seg::new(" panorama", DIM)],
    ) + 4;
    for f in Filter::ALL {
        let on = view.filter == f;
        let start = x;
        x = s.segs(
            x,
            0,
            &[Seg::new(f.label(), if on { HI } else { FAINT }).bold_if(on)],
        ) + 3;
        s.target(start, x - 3, 0, Target::Filter(f));
    }
    if let Some(why) = stale {
        let badge = Seg::new(" refresh failed ", ON_LIGHT)
            .bold()
            .on(status(&Status::Waiting));
        let why = format!(" {}", trunc(why, (s.w() - x - 49).max(0)));
        s.segs(x, 0, &[badge, Seg::new(why, FAINT)]);
    }
    let c = swarm.counts();
    let mut segs = vec![
        Seg::new(format!("? {}", c.questions), status(&Status::Waiting)).bold(),
        Seg::plain("  "),
        Seg::new(format!("! {}", c.blocked), status(&Status::Blocked)).bold(),
        Seg::plain("  "),
        Seg::new(format!("✗ {}", c.failed), status(&Status::Failed)).bold(),
        Seg::plain("    "),
        Seg::new(hhmm(Some(view.now), view.utc_offset), DIM),
    ];
    if s.w() < 60 {
        segs.drain(..segs.len() - 1);
    }
    let x = s.w() - width(&segs) - 1;
    s.segs(x, 0, &segs);
}
