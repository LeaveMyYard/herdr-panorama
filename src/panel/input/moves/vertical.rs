//! ↑/↓ in the focused column, and where a parked cursor resumes (Python `action_move`,
//! `_resume`).

use crate::panel::model::Panel;
use panorama_core::view::NEW_ROW;
use panorama_core::view::nav::nav;

impl Panel {
    /// ↑/↓ by `delta` in the focused column; from a parked cursor, the next shown row in that
    /// direction. Whether the cursor moved over orchestrators (the arrows' switch then waits
    /// for them to rest).
    pub(crate) fn move_cursor(&mut self, delta: i32) -> bool {
        let col = self.view.focus;
        let cols = nav(&self.swarm, &self.view);
        let Some(items) = cols.get(col).filter(|c| !c.is_empty()) else {
            return false;
        };
        let idx = match self
            .cur(col)
            .and_then(|c| items.iter().position(|i| i == c))
        {
            Some(i) => (i as i64 + i64::from(delta)).clamp(0, items.len() as i64 - 1) as usize,
            None => self.resume(col, items, delta),
        };
        let pick = items[idx].clone();
        self.select(col, &pick);
        col == 0
    }

    /// Where a parked cursor resumes: from the orchestrator it sits on (or was parked off) in
    /// the full list, to the next shown one in the arrow's direction; else the top.
    fn resume(&self, col: usize, items: &[String], delta: i32) -> usize {
        let order: Vec<&str> = self
            .swarm
            .orchestrators
            .iter()
            .map(|o| o.pane_id.as_str())
            .collect();
        let anchor = if col == 0 {
            self.view.orch.as_deref().or(self.parked.as_deref())
        } else {
            None
        };
        let Some(pos) = anchor.and_then(|a| order.iter().position(|p| *p == a)) else {
            return 0;
        };
        // + New agent ranks last; an id not in the list ranks as `missing`
        let rank = |x: &str, missing: i64| -> i64 {
            if x == NEW_ROW {
                order.len() as i64
            } else {
                order
                    .iter()
                    .position(|p| *p == x)
                    .map_or(missing, |i| i as i64)
            }
        };
        let pos = pos as i64;
        if delta > 0 {
            items
                .iter()
                .position(|x| rank(x, -1) > pos)
                .unwrap_or(items.len() - 1)
        } else {
            (0..items.len())
                .rev()
                .find(|&i| rank(&items[i], order.len() as i64) < pos)
                .unwrap_or(0)
        }
    }
}
