//! The mouse over the graph (Python `SwarmApp.hover`, `wheel`): the hover look, and a column
//! the wheel scrolls.

use super::Graph;
use panorama_core::view::Screen;

impl Graph {
    /// The mouse moved to `(x, y)` (`None`: it left): the card under it gets the hover look.
    /// Whether that changed anything.
    pub fn hover(&mut self, at: Option<(i32, i32)>, screen: &Screen) -> bool {
        let hit = at.and_then(|(x, y)| screen.card_at(x, y).cloned());
        if hit == self.view.hover {
            return false;
        }
        self.view.hover = hit;
        true
    }

    /// The wheel over `(x, y)` by `rows`: the column there scrolls and stops following the
    /// selection until it changes. Whether there was a column; if so, the actor repaints and
    /// calls its `painted` with `Painted::Wheel`, which holds the cells it slid.
    pub fn wheel(&mut self, x: i32, y: i32, rows: i32, screen: &Screen) -> bool {
        let Some(col) = screen
            .columns
            .iter()
            .find(|c| (c.x0..c.x1).contains(&x) && (c.y0..c.y1).contains(&y))
        else {
            return false;
        };
        let off = self.view.offsets.entry(col.column).or_insert(0);
        *off = (*off + rows).max(0);
        self.view.wheel.insert(col.column);
        true
    }
}
