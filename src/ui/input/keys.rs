//! The keys (Python `SwarmApp.BINDINGS`): what each one asks for.

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

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
