//! The cursor (Python `SwarmApp.action_move`, `_resume`, `_select`, `_point`,
//! `action_right`, `action_left`, `_shown`): selection is by identity, column by column.

mod columns;
mod identity;
mod vertical;

#[cfg(test)]
mod tests;
