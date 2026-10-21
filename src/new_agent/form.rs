//! The + New agent modal's state (Python `tui.NewAgentForm`, design 1C): the harness
//! carousel, the name (its agent name under it) and the working directory, a status row,
//! busy while the agent starts, and armed like every view (`ARM`, after it appears). Keys and
//! clicks in; a start or a cancel out. Nothing here does IO, except reading whether the
//! working directory exists on submit.

use super::text::Text;
use super::{name_ok, slugify};
use herdr_agent_kit::herdr_api::clean;
use panorama_core::view::new_agent::{Field, Form, HARNESSES, Modal, step};
use std::path::Path;
use std::time::{Duration, Instant};

/// Keys act this long after the form appears (Python `ARM_SECONDS`).
pub const ARM: Duration = Duration::from_millis(400);

/// A key the form reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FormKey {
    Char(char),
    Left,
    Right,
    Home,
    End,
    Backspace,
    Delete,
    Tab,
    BackTab,
    Enter,
    /// ctrl+s: start, from any field.
    Submit,
    Esc,
}

/// What the form asks for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Nothing beyond its own change (repaint).
    Changed,
    /// Nothing at all.
    Nothing,
    /// Closed without starting anything.
    Cancel,
    /// Start this agent; the form is busy until [`NewAgentForm::finished`].
    Start {
        name: String,
        cwd: Option<String>,
        kind: &'static str,
    },
}

/// The modal's state.
#[derive(Clone, Debug)]
pub struct NewAgentForm {
    /// The selected harness (an index into [`HARNESSES`]).
    pub harness: usize,
    pub name: Text,
    pub cwd: Text,
    pub focus: Field,
    pub status: Option<String>,
    /// The agent is starting: everything waits.
    pub busy: bool,
    armed_at: Instant,
}

impl NewAgentForm {
    /// The form on Claude Code, the name focused, `cwd` filled in, shown at `now`.
    pub fn new(cwd: &str, now: Instant) -> Self {
        Self {
            harness: 0,
            name: Text::default(),
            cwd: Text::new(cwd),
            focus: Field::Name,
            status: None,
            busy: false,
            armed_at: now + ARM,
        }
    }

    fn field(&mut self) -> &mut Text {
        match self.focus {
            Field::Name => &mut self.name,
            Field::Cwd => &mut self.cwd,
        }
    }

    /// A key at `now`.
    pub fn key(&mut self, k: FormKey, now: Instant) -> Outcome {
        if self.busy {
            return Outcome::Nothing; // the fields are disabled; Esc does not cancel a start
        }
        match k {
            FormKey::Esc => return Outcome::Cancel,
            FormKey::Submit => return self.submit(now),
            FormKey::Enter if self.focus == Field::Cwd => return self.submit(now),
            FormKey::Enter | FormKey::Tab | FormKey::BackTab => {
                self.focus = match self.focus {
                    Field::Name => Field::Cwd,
                    Field::Cwd => Field::Name,
                };
            }
            FormKey::Char(c) if !c.is_control() => self.field().insert(c),
            FormKey::Char(_) => return Outcome::Nothing,
            FormKey::Left => self.field().left(),
            FormKey::Right => self.field().right(),
            FormKey::Home => self.field().home(),
            FormKey::End => self.field().end(),
            FormKey::Backspace => self.field().back(),
            FormKey::Delete => self.field().delete(),
        }
        Outcome::Changed
    }

    /// A click at `(x, y)` on the modal as drawn: the carousel's ghosts and arrows step it.
    pub fn click(&mut self, x: i32, y: i32, modal: &Modal) -> Outcome {
        if self.busy {
            return Outcome::Nothing;
        }
        match modal.step_at(x, y) {
            Some(d) => {
                self.harness = step(self.harness, d);
                Outcome::Changed
            }
            None => Outcome::Nothing,
        }
    }

    /// Enter on the last field or ctrl+s: checked here, then started (Python `action_submit`).
    fn submit(&mut self, now: Instant) -> Outcome {
        let h = &HARNESSES[self.harness % HARNESSES.len()];
        if now < self.armed_at || !h.supported() {
            return Outcome::Nothing; // not armed yet; a harness that is coming soon starts nothing
        }
        if !name_ok(&slugify(&self.name.value)) {
            self.status = Some("The name needs at least one letter.".into());
            return Outcome::Changed;
        }
        let cwd = self.cwd.value.trim().to_owned();
        if !cwd.is_empty() && !Path::new(&cwd).is_dir() {
            self.status = Some(format!("No such directory: {}", clean(&cwd, 200)));
            return Outcome::Changed;
        }
        self.busy = true;
        self.status = Some(format!("starting {}…", h.name));
        Outcome::Start {
            name: self.name.value.clone(),
            cwd: (!cwd.is_empty()).then_some(cwd),
            kind: h.id,
        }
    }

    /// The start ended: `Ok(slug)` closes the form (its name, for the notice); an error stays
    /// on the status row and the form takes keys again.
    pub fn finished(&mut self, result: Result<String, String>) -> Option<String> {
        self.busy = false;
        match result {
            Ok(slug) => Some(slug),
            Err(e) => {
                self.status = Some(clean(&e, 300));
                None
            }
        }
    }

    /// What the modal draws.
    pub fn view(&self) -> FormView {
        FormView {
            slug: slugify(&self.name.value),
        }
    }

    /// The modal's picture of this state (`view` holds the agent name it shows).
    pub fn form<'a>(&'a self, view: &'a FormView) -> Form<'a> {
        let cursor = match self.focus {
            Field::Name => self.name.cursor,
            Field::Cwd => self.cwd.cursor,
        };
        Form {
            harness: self.harness,
            name: &self.name.value,
            slug: &view.slug,
            cwd: &self.cwd.value,
            focus: (!self.busy).then_some((self.focus, cursor)),
            status: self.status.as_deref(),
        }
    }
}

/// What the modal shows that the state derives.
pub struct FormView {
    pub slug: String,
}
