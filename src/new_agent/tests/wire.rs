//! The modal's terminal side: the keys it reads, the dimmed view under it, the popup's picture
//! and its started mark.

use crate::new_agent::form::{FormKey, NewAgentForm};
use crate::new_agent::keys::form_key;
use crate::new_agent::overlay::{SHADE, over};
use crate::new_agent::popup::{cwd, mark_started, picture};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use herdr_agent_kit::plugin_env::PluginEnv;
use panorama_core::view::Screen;
use panorama_core::view::colour::Rgb;
use std::time::Instant;

fn k(code: KeyCode, m: KeyModifiers) -> KeyEvent {
    KeyEvent::new(code, m)
}

#[test]
fn the_keys_the_form_reads() {
    let none = KeyModifiers::NONE;
    assert_eq!(
        form_key(k(KeyCode::Char('x'), none)),
        Some(FormKey::Char('x'))
    );
    assert_eq!(
        form_key(k(KeyCode::Char('X'), KeyModifiers::SHIFT)),
        Some(FormKey::Char('X'))
    );
    assert_eq!(
        form_key(k(KeyCode::Char('s'), KeyModifiers::CONTROL)),
        Some(FormKey::Submit)
    );
    assert_eq!(form_key(k(KeyCode::Char('a'), KeyModifiers::CONTROL)), None);
    assert_eq!(form_key(k(KeyCode::Char('a'), KeyModifiers::ALT)), None);
    for (code, want) in [
        (KeyCode::Left, FormKey::Left),
        (KeyCode::Right, FormKey::Right),
        (KeyCode::Up, FormKey::Up),
        (KeyCode::Down, FormKey::Down),
        (KeyCode::Home, FormKey::Home),
        (KeyCode::End, FormKey::End),
        (KeyCode::Backspace, FormKey::Backspace),
        (KeyCode::Delete, FormKey::Delete),
        (KeyCode::Tab, FormKey::Tab),
        (KeyCode::BackTab, FormKey::BackTab),
        (KeyCode::Enter, FormKey::Enter),
        (KeyCode::Esc, FormKey::Esc),
    ] {
        assert_eq!(form_key(k(code, none)), Some(want), "{code:?}");
    }
    let mut release = k(KeyCode::Enter, none);
    release.kind = KeyEventKind::Release;
    assert_eq!(form_key(release), None);
    assert_eq!(form_key(k(KeyCode::F(1), none)), None);
}

#[test]
fn the_view_under_the_modal_is_dimmed_and_the_modal_drawn_whole() {
    let mut view = Screen::new(100, 40);
    view.put(
        0,
        0,
        "abc",
        Rgb(200, 200, 200),
        Some(Rgb(40, 40, 40)),
        false,
    );
    let form = NewAgentForm::new("/work", Instant::now());
    let modal = panorama_core::view::new_agent::render(&form.form(&form.view()), 100, 40);
    over(&mut view, &modal.screen);
    let c = view.cell(0, 0);
    assert_eq!(
        (c.ch, c.fg, c.bg),
        (
            'a',
            Rgb(200, 200, 200).mix(SHADE, 0.55),
            Some(Rgb(40, 40, 40).mix(SHADE, 0.55))
        )
    );
    assert_eq!(
        view.cell(99, 39).bg,
        Some(SHADE),
        "a cell with no background takes the shade"
    );
    let (mx, my) = ((100 - 76) / 2, (40 - 27) / 2);
    for y in my..my + 27 {
        for x in mx..mx + 76 {
            assert_eq!(view.cell(x, y), modal.screen.cell(x, y), "({x}, {y})");
        }
    }
}

#[test]
fn the_popup_shows_the_same_modal_on_its_own_background() {
    let form = NewAgentForm::new("/work", Instant::now());
    let (s, modal) = picture(&form, 84, 31);
    assert_eq!(s.cell(0, 0).bg, Some(SHADE));
    assert!(s.text().contains("+ New agent") && s.text().contains("/work"));
    assert!(!modal.steps.is_empty(), "its carousel takes clicks");
}

#[test]
fn the_popup_says_it_started_and_takes_its_directory() {
    let dir = tempfile::tempdir().unwrap();
    let mark = dir.path().join("state").join("new-agent-k.started");
    let t0 = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs_f64()
        - 1.0;
    mark_started(&mark);
    assert!(crate::panel::act::new_agent::started_since(&mark, t0));
    let env = |vars: &[(&str, &str)]| {
        PluginEnv::from_vars(vars.iter().copied(), Some("/home/op".into()), 1000)
    };
    assert_eq!(cwd(&env(&[("PANORAMA_NEW_AGENT_CWD", "/work")])), "/work");
    assert_eq!(cwd(&env(&[])), "/home/op");
    assert_eq!(cwd(&env(&[("PANORAMA_NEW_AGENT_CWD", "")])), "/home/op");
}

#[test]
fn a_new_panel_gets_the_plugins_own_variables_that_are_set() {
    let env = PluginEnv::from_vars(
        [
            ("HERDR_PLUGIN_ID", "p"),
            ("HERDR_PLUGIN_ROOT", "/r"),
            ("HERDR_PLUGIN_STATE_DIR", ""),
            ("HERDR_SOCKET_PATH", "/s"),
        ],
        None,
        1000,
    );
    assert_eq!(
        crate::new_agent::real::plugin_env(&env),
        [
            ("HERDR_PLUGIN_ID".to_owned(), "p".to_owned()),
            ("HERDR_PLUGIN_ROOT".to_owned(), "/r".to_owned())
        ]
    );
}

#[test]
fn the_popup_holds_the_whole_modal_and_the_manifest_says_the_same_size() {
    use crate::panel::act::new_agent::POPUP;
    use panorama_core::view::new_agent::{H, W};
    // room for the 1C modal inside herdr's frame (one cell each side)
    assert!(
        i32::from(POPUP.0) >= W + 2 && i32::from(POPUP.1) >= H + 2,
        "{POPUP:?}"
    );
    let toml =
        std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/herdr-plugin.toml")).unwrap();
    // the new-agent pane's block (no TOML parser here: its `key = value` lines)
    let block = toml
        .split("[[panes]]")
        .find(|b| b.contains("id = \"new-agent\""))
        .unwrap();
    let int = |k: &str| {
        block.lines().find_map(|l| {
            l.strip_prefix(k)?
                .trim()
                .strip_prefix('=')?
                .trim()
                .parse::<u16>()
                .ok()
        })
    };
    assert_eq!(
        (int("width"), int("height")),
        (Some(POPUP.0), Some(POPUP.1))
    );
}
