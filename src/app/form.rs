//! The + New agent modal in the app (Python `SwarmApp.action_new_agent` pushing
//! `NewAgentForm`): open over the view, its keys and clicks, its start and its end. The view
//! goes on refreshing behind it.

use super::driver::Driver;
use crate::new_agent::form::{FormKey, NewAgentForm, Outcome};
use crate::new_agent::keys::form_key;
use crate::new_agent::overlay::over;
use crate::ui::input::Toast;
use crossterm::event::{Event, MouseButton, MouseEventKind};
use panorama_core::view::Screen;
use panorama_core::view::new_agent::{Modal, render};
use std::time::Instant;

/// The modal and its picture as last drawn (its click targets).
pub struct OpenForm {
    pub form: NewAgentForm,
    modal: Option<Modal>,
}

/// What the actor does after a key or a click on the modal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FormStep {
    /// Nothing changed.
    Nothing,
    /// Draw again.
    Repaint,
    /// Start this agent (the modal shows it starting).
    Start {
        name: String,
        cwd: Option<String>,
        kind: &'static str,
    },
    /// The modal closed without starting anything.
    Closed,
}

impl Driver {
    /// The modal over the view, `cwd` its directory (nothing when one is open already).
    pub fn open_form(&mut self, cwd: &str, now: Instant) {
        if self.form.is_none() {
            self.form = Some(OpenForm {
                form: NewAgentForm::new(cwd, now),
                modal: None,
            });
        }
    }

    fn step(&mut self, out: Outcome) -> FormStep {
        match out {
            Outcome::Nothing => FormStep::Nothing,
            Outcome::Changed => FormStep::Repaint,
            Outcome::Cancel => {
                self.form = None;
                FormStep::Closed
            }
            Outcome::Start { name, cwd, kind } => FormStep::Start { name, cwd, kind },
        }
    }

    /// A terminal event while the modal is open: its keys and left clicks go to the form, the
    /// rest of the pointer to nothing, so none reaches the view behind (`Some`, what the modal
    /// did). `None` while no modal is open, or for an event that is no key or pointer (a
    /// resize, focus): the view's then.
    pub fn modal_input(&mut self, ev: &Event, now: Instant) -> Option<FormStep> {
        self.form.as_ref()?;
        Some(match ev {
            Event::Key(k) => match form_key(*k) {
                Some(k) => self.form_key(k, now),
                None => FormStep::Nothing,
            },
            Event::Mouse(m) if m.kind == MouseEventKind::Down(MouseButton::Left) => {
                self.form_click(i32::from(m.column), i32::from(m.row))
            }
            Event::Mouse(_) => FormStep::Nothing,
            _ => return None,
        })
    }

    /// A key while the modal is open.
    pub fn form_key(&mut self, k: FormKey, now: Instant) -> FormStep {
        let Some(open) = self.form.as_mut() else {
            return FormStep::Nothing;
        };
        let out = open.form.key(k, now);
        self.step(out)
    }

    /// A click while the modal is open: on the carousel's steps it switches the harness; the
    /// view behind takes none.
    pub fn form_click(&mut self, x: i32, y: i32) -> FormStep {
        let Some(open) = self.form.as_mut() else {
            return FormStep::Nothing;
        };
        let Some(modal) = &open.modal else {
            return FormStep::Nothing;
        };
        let out = open.form.click(x, y, modal);
        self.step(out)
    }

    /// The start ended: `Some(name)` when the agent runs (the modal closed); an error stays
    /// on the modal's status row.
    pub fn form_started(&mut self, result: Result<String, String>) -> Option<String> {
        let open = self.form.as_mut()?;
        let name = open.form.finished(result)?;
        self.form = None;
        Some(name)
    }

    /// The modal's start ended (Python `_new_agent_done`): when it runs, the modal closes and
    /// "started <name>" shows; whether the view closes then (a popup, which would hide the
    /// new tab). An error stays on the modal's status row.
    pub fn agent_started(&mut self, result: Result<String, String>, now: Instant) -> bool {
        let Some(name) = self.form_started(result) else {
            return false;
        };
        self.toasts.push(
            Toast::new(Some("new agent"), format!("started {name}"), 6),
            now,
        );
        !self.app.home_mode
    }

    /// The modal, when open, over the dimmed view; its picture is kept for its clicks.
    pub fn form_over(&mut self, screen: &mut Screen) {
        let Some(open) = self.form.as_mut() else {
            return;
        };
        let view = open.form.view();
        let modal = render(&open.form.form(&view), screen.w(), screen.h());
        over(screen, &modal.screen);
        open.modal = Some(modal);
    }
}
