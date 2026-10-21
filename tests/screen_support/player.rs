//! A headless `panorama tui` (Python's capture `tools/capture_goldens.py play`, for the
//! `swarm` app): a run's own steps through the app's [`Driver`], each snap the screen the
//! terminal would show (the notices over the view), with the unseen marks the capture drew.

use panorama::app::driver::Driver;
use panorama::app::model::App;
use panorama::ui::frame::Frame;
use panorama::ui::input::{Effect, Key, Painted};
use panorama_core::model::Swarm;
use panorama_core::view::{Marks, PanelSettings, Screen, Target, Tokens};
use serde_json::Value;
use std::time::{Duration, Instant};

/// A Textual key name as the capture pressed it.
pub fn key(name: &str) -> Key {
    match name {
        "down" => Key::Move(1),
        "up" => Key::Move(-1),
        "right" => Key::Right,
        "left" => Key::Left,
        "enter" => Key::Enter,
        "escape" => Key::Escape,
        "question_mark" => Key::Help,
        "f" => Key::Filter,
        "g" => Key::Goto,
        "t" => Key::Transcript,
        "a" => Key::Answer,
        "i" => Key::Inbox,
        "n" => Key::NewAgent,
        "q" => Key::Quit,
        "z" => Key::Zoom,
        other => panic!("a key this player does not press: {other}"),
    }
}

/// A target as the capture wrote it: `[kind, [ident…]]`.
fn target(v: &Value) -> Option<Target> {
    let ident = |i: usize| v[1][i].as_str().unwrap().to_owned();
    Some(match v[0].as_str()? {
        "orch" => Target::Orch(ident(0)),
        other => panic!("a hover this player does not make: {other}"),
    })
}

/// One run playing.
pub struct Player<'a> {
    run: &'a Value,
    unseen: &'a Value,
    pub d: Driver,
    frame: f64,
    /// The arming clock: well after the first refresh, as Python's held clock (`disarm` is
    /// then a no-op: nothing changes under the keys in a capture).
    at: Instant,
    /// What the actor would have carried out, in order (the herdr side's is P4 8c's).
    pub effects: Vec<Effect>,
}

impl<'a> Player<'a> {
    /// `run` of a screens file (`unseen`: its file's marks by snap), after its first
    /// refresh with `swarm`.
    pub fn new(run: &'a Value, unseen: &'a Value, mut swarm: Swarm) -> Self {
        swarm.docked = run["docked"].as_str().map(Into::into);
        let t0 = Instant::now();
        let home = run["home_mode"].as_bool().unwrap_or(false);
        let mut app = App::new(run["focus_pane"].as_str().map(Into::into), home, t0);
        app.apply(swarm, Marks::new(), t0);
        let mut p = Self {
            run,
            unseen,
            d: Driver::new(app),
            frame: 0.0,
            at: t0 + Duration::from_secs(5),
            effects: vec![],
        };
        p.paint(Painted::Refresh);
        p
    }

    /// The frame as the capture held it: the run's clock, TZ=UTC, no herdr config (herdr's
    /// default theme, panorama's own colours, the open key unbound).
    fn look(&self) -> Frame {
        Frame {
            now: self.run["now"].as_f64().unwrap(),
            frame: self.frame,
            utc_offset: 0,
            theme: Tokens::builtin("catppuccin"),
            settings: PanelSettings::default(),
            open_key: ("pfx+spc".into(), "pfx+spc".into()),
        }
    }

    fn paint(&mut self, cause: Painted) -> Screen {
        let (w, h) = (
            self.run["size"][0].as_i64().unwrap() as i32,
            self.run["size"][1].as_i64().unwrap() as i32,
        );
        let look = self.look();
        self.d
            .paint(w, h, &look, cause, self.at)
            .expect("a sized pane")
    }

    /// Plays the steps; each snap's screen, by name. It stops before the first snap in `skip`
    /// (one this player does not make): the steps that lead there are that snap's.
    pub fn play(mut self, skip: &[&str]) -> Vec<(String, Screen)> {
        let mut snaps = vec![];
        let steps = self.run["steps"].as_array().unwrap();
        // the steps up to the first snap skipped (the ones leading to it are its own)
        let first_skipped = steps
            .iter()
            .position(|st| st["do"] == "snap" && skip.contains(&st["name"].as_str().unwrap()));
        let end = first_skipped.map_or(steps.len(), |k| {
            steps[..k]
                .iter()
                .rposition(|st| st["do"] == "snap")
                .map_or(0, |i| i + 1)
        });
        for step in &steps[..end] {
            match step["do"].as_str().unwrap() {
                "frame" => {
                    self.frame = step["n"].as_f64().unwrap_or(0.0);
                    self.paint(Painted::Other);
                }
                "press" => {
                    for k in step["keys"].as_array().unwrap() {
                        self.press(key(k.as_str().unwrap()));
                    }
                }
                "disarm" => {}
                "hover" => {
                    self.d.app.graph.view.hover = target(&step["target"]);
                    self.paint(Painted::Other);
                }
                "blur" => {
                    self.d.app.pane_focused = false;
                    self.paint(Painted::Other);
                }
                "select" => {
                    let v = &mut self.d.app.graph.view;
                    v.thread = step["thread"].as_str().map(Into::into);
                    v.agent = step["agent"].as_str().map(Into::into);
                    v.focus = step["focus"].as_u64().unwrap() as usize;
                    self.paint(Painted::Other);
                }
                "snap" => {
                    let name = step["name"].as_str().unwrap();
                    self.d.app.graph.view.unseen =
                        serde_json::from_value(self.unseen[name].clone()).unwrap();
                    snaps.push((name.to_owned(), self.paint(Painted::Other)));
                }
                other => panic!("a step this player does not take: {other}"),
            }
        }
        snaps
    }

    /// A key, and what it asks for: a notice goes over the view (the rest is the herdr
    /// side's, P4 8c).
    fn press(&mut self, k: Key) {
        for e in self.d.key(k, self.at) {
            if let Effect::Toast(t) = &e {
                self.d.toasts.push(t.clone(), self.at);
            }
            self.effects.push(e);
        }
        self.paint(Painted::Other);
    }
}
