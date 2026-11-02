//! Harness names occupy the existing title area; all focus targets retain pane identity.
use panorama_core::model::Swarm;
use panorama_core::view::{Filter, Target, View, render_dock, render_graph};
use panorama_core::view::nav::{nav};
use serde_json::json;

fn swarm() -> Swarm {
    serde_json::from_value(json!({"orchestrators":[
        {"pane_id":"claude-pane","tab_id":"c","tab_label":"same title","session_id":null,
         "harness":"claude", "herdr_status":"busy", "tab_no":1},
        {"pane_id":"codex-pane","tab_id":"x","tab_label":"same title","session_id":null,
         "harness":"codex", "herdr_status":"blocked", "tab_no":2}
    ]})).unwrap()
}

#[test]
fn mixed_harnesses_are_named_and_click_targets_still_identify_exact_panes() {
    let swarm = swarm();
    let view = View { orch:Some("claude-pane".into()), ..View::default() };
    for (dock, width) in [(true,46),(true,30),(false,70),(false,160)] {
        let screen = if dock {render_dock(&swarm,&view,width,32,None,"empty")}
            else {render_graph(&swarm,&view,width,32,None,"empty")};
        let text = screen.text();
        assert!(text.contains("Claude Code"), "{dock}/{width}: {text}");
        assert!(text.contains("Codex"), "{dock}/{width}: {text}");
        for (name,pane) in [("Claude Code","claude-pane"),("Codex","codex-pane")] {
            let (y,line)=text.lines().enumerate().find(|(_,l)|l.contains(name)).unwrap();
            let x=line.chars().collect::<String>().find(name).unwrap();
            // Unicode byte offsets differ: count the characters preceding the label.
            let x=line[..x].chars().count();
            assert_eq!(screen.target_at(x as i32,y as i32),Some(&Target::Orch(pane.into())));
        }
    }
}

#[test]
fn mixed_navigation_and_filters_use_pane_ids() {
    let sw = swarm();
    let mut view = View::default();
    assert_eq!(nav(&sw, &view)[0], ["claude-pane", "codex-pane", panorama_core::view::NEW_ROW]);
    view.filter = Filter::Needs;
    assert_eq!(nav(&sw, &view)[0], ["codex-pane", panorama_core::view::NEW_ROW]);
}
