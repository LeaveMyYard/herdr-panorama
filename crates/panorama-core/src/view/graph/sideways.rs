//! The line above the wide graph's columns (Python `_wide`'s first lines): what is scrolled
//! off on the left (the path to the columns shown) and how many columns are off on the right.

use super::layout::{Columns, col_label};
use crate::model::Root;
use crate::view::agent_text::name;
use crate::view::nav::find_agent;
use crate::view::palette::{DIM, FAINT, FG};
use crate::view::screen::Screen;
use crate::view::seg::Seg;
use crate::view::text::trunc;

/// Row `y`'s words for `cols` off either side; `path`: the selected agent and the agents
/// that spawned it, outermost first.
pub fn line(s: &mut Screen, y: i32, cols: &Columns, o: Option<&Root>, path: &[String]) {
    let (lo, hi, n) = (cols.lo, cols.hi, cols.len());
    if lo > 0 {
        // the breadcrumbs: the root, then the selected agent's path, a column each
        let mut names = vec![o.map_or(String::new(), |o| o.name().to_owned())];
        names.extend(
            path.iter()
                .filter_map(|id| find_agent(o, Some(id)))
                .map(name),
        );
        names.truncate(lo);
        let mut segs = vec![Seg::new("‹ ", DIM).bold()];
        for (i, name) in names.iter().enumerate() {
            let last = i == names.len() - 1;
            segs.push(Seg::new(trunc(name, 28), if last { FG } else { DIM }));
            segs.push(Seg::new(if last { "" } else { " › " }, FAINT));
        }
        let plural = if lo > 1 { "s" } else { "" };
        segs.push(Seg::new(format!("   {lo} column{plural} left"), FAINT));
        s.segs(2, y, &segs);
    }
    if hi < n - 1 {
        let k = n - 1 - hi;
        let what = if k == 1 {
            col_label(hi + 1)
        } else {
            format!("Depth {}–{}", hi + 1, n - 1)
        };
        let plural = if k > 1 { "s" } else { "" };
        let text = format!("{what}  {k} column{plural} right ");
        let x = s.w() - 2 - text.chars().count() as i32 - 1;
        s.segs(x, y, &[Seg::new(text, FAINT), Seg::new("›", DIM).bold()]);
    }
}
