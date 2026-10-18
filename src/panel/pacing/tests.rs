use super::*;
use panorama_core::view::Unseen;
use panorama_core::view::palette::FG;

fn swarm(status: &str) -> Swarm {
    serde_json::from_value(serde_json::json!({"orchestrators": [{"pane_id": "p", "tab_id": null,
        "tab_label": "x", "session_id": null,
        "threads": [{"id": "t", "name": "n", "source": "turn", "agents": [{"id": "a", "status": status}]}]}]}))
    .unwrap()
}

#[test]
fn a_hidden_tab_or_a_still_screen_never_ticks() {
    let none = Marks::new();
    assert!(ticking(true, &swarm("run"), &none));
    assert!(!ticking(false, &swarm("run"), &none)); // hidden
    assert!(!ticking(true, &swarm("done"), &none)); // nothing moves
    let marks: Marks = [("a".to_owned(), Unseen::Done)].into();
    assert!(ticking(true, &swarm("done"), &marks)); // an unseen pulse moves
}

#[test]
fn a_tick_draws_on_a_new_whole_frame_or_while_colours_glide() {
    let mut p = Pacing::default();
    assert!(p.due(0.0));
    let still = Screen::new(2, 1);
    p.drawn(4.2, &still);
    assert!(!p.due(4.9)); // the same whole frame, nothing glides
    assert!(p.due(5.0));
    let mut gliding = Screen::new(2, 1);
    gliding.put_seg(
        0,
        0,
        &panorama_core::view::seg::Seg::new("x", FG).fg_moving(true),
    );
    p.drawn(4.2, &gliding);
    assert!(p.due(4.3));
}
