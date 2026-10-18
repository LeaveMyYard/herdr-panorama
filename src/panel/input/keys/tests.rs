use super::*;

fn k(code: KeyCode) -> Option<Key> {
    key(KeyEvent::new(code, KeyModifiers::NONE))
}

#[test]
fn the_bindings_are_pythons() {
    assert_eq!(k(KeyCode::Up), Some(Key::Move(-1)));
    assert_eq!(k(KeyCode::Char('j')), Some(Key::Move(1)));
    assert_eq!(k(KeyCode::Char('l')), Some(Key::Right));
    assert_eq!(k(KeyCode::Char('h')), Some(Key::Left));
    assert_eq!(k(KeyCode::Char('?')), Some(Key::Help));
    assert_eq!(k(KeyCode::Char('x')), None);
    assert_eq!(
        key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::CONTROL)),
        None
    );
    let mut release = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
    release.kind = KeyEventKind::Release;
    assert_eq!(key(release), None);
}
