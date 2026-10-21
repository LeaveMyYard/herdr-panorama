//! The new-agent modal against design 1C "Coming soon" (`design_new_agent.json`, captured by
//! node from the design's own `herdr-tui.js`): the modal's cells for every harness in the
//! centre (Claude Code, then each one not supported yet with its neighbours' "Coming soon"),
//! its click targets, the harness table, and each field focused as the design's `input()`
//! draws it.

mod support;

use panorama_core::view::Rgb;
use panorama_core::view::new_agent::{Field, Form, H, HARNESSES, Step, W, render};
use serde_json::Value;
use support::{Shown, assert_same, cells, fixture, golden};

/// The modal's place on the design's 96 × 32 canvas.
const MX: usize = 10;
const MY: usize = 3;
const NAME: &str = "fix tab prefix stacking";
const SLUG: &str = "fix-tab-prefix-stacking";
const CWD: &str = "~/Code/herdr";

fn form(harness: usize, focus: Option<(Field, usize)>) -> Form<'static> {
    Form {
        harness,
        name: NAME,
        slug: SLUG,
        cwd: CWD,
        focus,
        status: None,
        carousel: false,
    }
}

fn crop(
    rows: &[Vec<Shown>],
    x: std::ops::Range<usize>,
    y: std::ops::Range<usize>,
) -> Vec<Vec<Shown>> {
    rows[y].iter().map(|r| r[x.clone()].to_vec()).collect()
}

#[test]
fn the_harness_table_is_the_design_s() {
    let want = &fixture("design_new_agent.json")["harness"];
    assert_eq!(want.as_array().unwrap().len(), HARNESSES.len());
    for (h, w) in HARNESSES.iter().zip(want.as_array().unwrap()) {
        assert_eq!(
            (h.id, h.name, h.cmd),
            (
                w["id"].as_str().unwrap(),
                w["name"].as_str().unwrap(),
                w["cmd"].as_str().unwrap()
            )
        );
        assert_eq!(Some(h.col), Rgb::parse(w["col"].as_str().unwrap()));
        let big: Vec<&str> = w["big"]
            .as_array()
            .unwrap()
            .iter()
            .map(|l| l.as_str().unwrap())
            .collect();
        assert_eq!(h.big.to_vec(), big, "{}", h.id);
    }
}

#[test]
fn every_harness_in_the_centre_is_the_design_s_modal_cell_for_cell() {
    let file = fixture("design_new_agent.json");
    for frame in file["frames"].as_array().unwrap() {
        let sel = frame["sel"].as_u64().unwrap() as usize;
        let want = crop(
            &golden(&frame["rows"]),
            MX..MX + W as usize,
            MY..MY + H as usize,
        );
        let m = render(&form(sel, None), W, H);
        assert_same(frame["id"].as_str().unwrap(), &want, &cells(&m.screen));
        let hits: Vec<Step> = frame["hits"].as_array().unwrap().iter().map(step).collect();
        assert_eq!(m.steps, hits, "{}", frame["id"]);
    }
}

fn step(h: &Value) -> Step {
    let n = |k: &str| h[k].as_i64().unwrap() as i32;
    Step {
        x: n("x") - MX as i32,
        y: n("y") - MY as i32,
        w: n("w"),
        h: n("h"),
        d: h["act"]["d"].as_i64().unwrap() as i32,
    }
}

#[test]
fn a_focused_field_is_the_design_s_focused_input_with_its_cursor() {
    let file = fixture("design_new_agent.json");
    let inside = 1..W as usize - 1; // the frame's inside: the input was drawn on a bare panel
    let name = golden(&file["input"]["name"]);
    let got = cells(&render(&form(0, Some((Field::Name, NAME.chars().count()))), W, H).screen);
    assert_same(
        "name",
        &crop(&name, inside.clone(), 0..6),
        &crop(&got, inside.clone(), 14..20),
    );
    let cwd = golden(&file["input"]["cwd"]);
    let got = cells(&render(&form(0, Some((Field::Cwd, CWD.chars().count()))), W, H).screen);
    // its sub-line row is the status row, empty here
    assert_same(
        "cwd",
        &crop(&cwd, inside.clone(), 0..5),
        &crop(&got, inside, 20..25),
    );
}

#[test]
fn the_palette_is_the_design_s() {
    use panorama_core::view::new_agent::look::*;
    let c = &fixture("design_new_agent.json")["C"];
    let named = [
        ("bg", BG),
        ("panel", PANEL),
        ("border", BORDER),
        ("text", TEXT),
        ("muted", MUTED),
        ("dim", DIM),
        ("faint", FAINT),
        ("accent", ACCENT),
        ("warn", WARN),
        ("field", FIELD),
    ];
    for (k, v) in named {
        assert_eq!(Rgb::parse(c[k].as_str().unwrap()), Some(v), "{k}");
    }
}
