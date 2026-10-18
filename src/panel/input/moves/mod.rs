//! The cursor (Python `SwarmApp.action_move`, `_resume`, `_select`, `_point`,
//! `action_right`, `action_left`, `_shown`): selection is by identity, column by column. It is
//! the shared [`crate::ui::graph::Graph`]'s, → and ← the shared [`crate::ui::input::Actions`]'
//! (the panel's docked list keeps one column through its `Host::dock`); the panel's tests of
//! the cursor stay here.

#[cfg(test)]
mod tests;
