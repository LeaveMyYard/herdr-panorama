//! The app's input (Python `SwarmApp`'s bindings and actions): the shared keys and clicks
//! ([`crate::ui::input`]) with what the app decides itself: Enter on an orchestrator goes to
//! its pane only in the home view, `g` goes there and closes a popup, a filter applies here,
//! Esc closes only a popup, z zooms the home view, and the wheel scrolls the columns sideways.
//! It returns the [`Effect`]s its actor carries out; nothing here does IO.

use super::model::App;
use crate::ui::input::actions::HELP;
use crate::ui::input::{Actions, Effect, Host, Key, Painted, Toast};
use panorama_core::view::nav::{default_children, nav};
use panorama_core::view::{Filter, Screen};
use std::time::Instant;

impl Host for App {
    fn armed(&self, now: Instant) -> bool {
        App::armed(self, now)
    }

    fn click_ready(&self, cell: (i32, i32), now: Instant) -> bool {
        self.clicks.ready(cell, now)
    }

    fn dock(&self) -> bool {
        self.dock
    }

    fn enter_goes_to_pane(&self) -> bool {
        self.home_mode
    }

    /// g: the selected orchestrator's pane, its agents seen at once (Python `action_goto`
    /// without the moving cockpit, Q187). The actor focuses it; a popup then closes.
    fn goto(&mut self, _switch: bool, now: Instant) -> Vec<Effect> {
        let mut out = vec![];
        if !App::armed(self, now) {
            return out;
        }
        let Some(o) = self.selection().0.cloned() else {
            return out;
        };
        self.seen(o.agents().map(|a| a.id.clone()).collect(), &mut out);
        if let Some(machine) = o.machine.clone() {
            let machine_id = o.machine_id.clone().unwrap_or_else(|| machine.clone());
            out.push(Effect::GotoRemote {
                machine_id,
                machine,
                pane: o.pane_id,
                label: o.tab_label,
            });
        } else {
            out.push(Effect::Goto {
                pane: o.pane_id,
                label: o.tab_label,
                keep_focus: false,
                seen: vec![],
            });
        }
        out
    }

    /// n: + New agent in the selected orchestrator's directory, else any orchestrator's
    /// (the actor falls back to the home directory).
    fn new_agent(&mut self) -> Vec<Effect> {
        let cwd = self
            .selection_orch_cwd()
            .or_else(|| self.swarm.orchestrators.iter().find_map(|o| o.cwd.clone()));
        vec![Effect::NewAgent { cwd }]
    }

    /// A filter chosen here: the selection it hides is parked, and with no orchestrator
    /// selected the first shown one is (Python `_apply_filter`).
    fn set_filter(&mut self, f: Filter, _now: Instant) -> Vec<Effect> {
        self.view.filter = f;
        self.view.wheel.clear();
        self.resolve();
        let first = nav(&self.swarm, &self.view)[0].first().cloned();
        if self.view.orch.is_none()
            && let Some(first) = first
        {
            let g = &mut self.graph;
            g.view.orch = Some(first);
            default_children(&g.swarm, &mut g.view);
        }
        vec![]
    }
}

impl App {
    /// The cursor's orchestrator's directory, if it has one.
    fn selection_orch_cwd(&self) -> Option<String> {
        let o = panorama_core::view::nav::find_orch(&self.swarm, self.view.orch.as_deref());
        o.and_then(|o| o.cwd.clone())
    }

    /// What `key` does now (Python `SwarmApp.BINDINGS`).
    pub fn on_key(&mut self, key: Key, now: Instant) -> Vec<Effect> {
        match key {
            Key::Move(delta) => {
                self.move_cursor(delta);
                vec![]
            }
            Key::Right => {
                self.right();
                vec![]
            }
            Key::Left => {
                self.left();
                vec![]
            }
            Key::Enter => self.enter(now),
            Key::Goto => Host::goto(self, false, now),
            Key::Transcript => self.transcript(now),
            Key::Answer => self.answer(now),
            Key::Inbox => self.inbox(now),
            Key::Filter => self.next_filter(now),
            Key::Help => vec![Effect::Toast(Toast::new(Some("keys"), HELP, 10))],
            Key::NewAgent => Host::new_agent(self),
            Key::Quit => vec![Effect::Quit],
            // the home tab is not dismissed by a stray Esc; q still quits
            Key::Escape if !self.home_mode => vec![Effect::Quit],
            Key::Escape => vec![],
            Key::Zoom if self.home_mode => vec![Effect::Zoom],
            Key::Zoom => vec![],
        }
    }

    /// The columns scrolled sideways by `cols` from where the last paint (`screen`) started
    /// them; the focus stays (Python `hwheel`). The actor repaints with [`Painted::Wheel`].
    pub fn hwheel(&mut self, cols: i32, screen: &Screen) -> Painted {
        let from = self.view.hcol.unwrap_or(screen.hlo) as i32;
        self.view.hcol = Some((from + cols).max(0) as usize);
        Painted::Wheel
    }

    /// After a paint `old` → `new` because of `cause`: the cells whose click target changed
    /// wait before a click acts there (a refresh's for a second, the wheel's briefly).
    pub fn stamp(&mut self, old: Option<&Screen>, new: &Screen, cause: Painted, now: Instant) {
        let none = Screen::new(0, 0);
        let old = old.unwrap_or(&none);
        match cause {
            Painted::Refresh => self.clicks.refreshed(old, new, now),
            Painted::Wheel => self.clicks.scrolled(old, new, now),
            Painted::Other => {}
        }
    }
}
