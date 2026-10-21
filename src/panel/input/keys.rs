//! The panel's keys: what each one does here (the bindings are [`crate::ui::input::keys`]'s).

use super::actions::HELP;
use super::effect::{Effect, Toast};
use crate::panel::model::Panel;
use crate::ui::input::Actions;
pub use crate::ui::input::keys::{Key, key};
#[cfg(test)]
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use std::time::Instant;

impl Panel {
    /// What `key` does now.
    pub fn on_key(&mut self, key: Key, now: Instant) -> Vec<Effect> {
        match key {
            Key::Move(delta) => self.arrow(delta, now),
            Key::Right => {
                self.right();
                vec![]
            }
            Key::Left => {
                self.left();
                vec![]
            }
            Key::Enter => self.enter(now),
            Key::Goto => self.goto(false, now),
            Key::Transcript => self.transcript(now),
            Key::Answer => self.answer(now),
            Key::Inbox => self.inbox(now),
            Key::Filter => self.next_filter(now),
            Key::Help => vec![Effect::Toast(Toast::new(Some("keys"), HELP, 10))],
            Key::NewAgent => self.new_agent(),
            Key::Quit => vec![Effect::Quit],
            // the home tab's panel is not dismissed by a stray Esc; a panel cannot zoom
            Key::Escape | Key::Zoom => vec![],
        }
    }
}

#[cfg(test)]
mod tests;
