//! A terminal key as the + New agent form reads it.

use super::form::FormKey;
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

/// The form's key for `ev`; `None` for one it does not read (and for releases).
pub fn form_key(ev: KeyEvent) -> Option<FormKey> {
    if ev.kind == KeyEventKind::Release {
        return None;
    }
    let ctrl = ev.modifiers.contains(KeyModifiers::CONTROL);
    if ev.modifiers.contains(KeyModifiers::ALT) {
        return None;
    }
    Some(match ev.code {
        KeyCode::Char('s') if ctrl => FormKey::Submit,
        KeyCode::Char(_) if ctrl => return None,
        KeyCode::Char(c) => FormKey::Char(c),
        KeyCode::Left => FormKey::Left,
        KeyCode::Right => FormKey::Right,
        KeyCode::Up => FormKey::Up,
        KeyCode::Down => FormKey::Down,
        KeyCode::Home => FormKey::Home,
        KeyCode::End => FormKey::End,
        KeyCode::Backspace => FormKey::Backspace,
        KeyCode::Delete => FormKey::Delete,
        KeyCode::Tab => FormKey::Tab,
        KeyCode::BackTab => FormKey::BackTab,
        KeyCode::Enter => FormKey::Enter,
        KeyCode::Esc => FormKey::Esc,
        _ => return None,
    })
}
