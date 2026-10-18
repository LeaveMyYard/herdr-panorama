//! The panel's keys (Python `SwarmApp.BINDINGS`): what each one asks for.

use super::actions::HELP;
use super::effect::{Effect, Toast};
use crate::panel::model::Panel;
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use std::time::Instant;

/// What a key asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    /// ↑/k, ↓/j.
    Move(i32),
    /// →/l.
    Right,
    /// ←/h.
    Left,
    Enter,
    /// g: go to the pane.
    Goto,
    /// t: the transcript's path.
    Transcript,
    /// a: answer.
    Answer,
    /// i: the inbox.
    Inbox,
    /// f: the next filter.
    Filter,
    /// ?: the keys.
    Help,
    /// n: + New agent.
    NewAgent,
    /// q: close.
    Quit,
    Escape,
    /// z: zoom (nothing in a panel).
    Zoom,
}

/// The key's meaning; `None` for one the panel does not bind (and for releases).
pub fn key(ev: KeyEvent) -> Option<Key> {
    if ev.kind == KeyEventKind::Release
        || ev
            .modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
    {
        return None;
    }
    Some(match ev.code {
        KeyCode::Up | KeyCode::Char('k') => Key::Move(-1),
        KeyCode::Down | KeyCode::Char('j') => Key::Move(1),
        KeyCode::Right | KeyCode::Char('l') => Key::Right,
        KeyCode::Left | KeyCode::Char('h') => Key::Left,
        KeyCode::Enter => Key::Enter,
        KeyCode::Char('g') => Key::Goto,
        KeyCode::Char('t') => Key::Transcript,
        KeyCode::Char('a') => Key::Answer,
        KeyCode::Char('i') => Key::Inbox,
        KeyCode::Char('f') => Key::Filter,
        KeyCode::Char('?') => Key::Help,
        KeyCode::Char('n') => Key::NewAgent,
        KeyCode::Char('q') => Key::Quit,
        KeyCode::Esc => Key::Escape,
        KeyCode::Char('z') => Key::Zoom,
        _ => return None,
    })
}

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
