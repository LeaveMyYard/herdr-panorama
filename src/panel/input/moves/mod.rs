//! The cursor (Python `SwarmApp.action_move`, `_resume`, `_select`, `_point`,
//! `action_right`, `action_left`, `_shown`): selection is by identity, column by column. It is
//! the shared [`crate::ui::graph::Graph`]'s; the panel adds the docked list's one column, and
//! its tests of the cursor stay here.

use crate::panel::model::Panel;

impl Panel {
    /// →: into the next column; the docked list has one, so → does nothing there.
    pub(crate) fn right(&mut self) {
        if !self.input.dock {
            self.graph.right();
        }
    }
}

#[cfg(test)]
mod tests;
