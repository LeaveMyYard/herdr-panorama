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
    /// ←: the previous harness on the carousel, else the text cursor.
    Left,
    /// →: the next harness on the carousel, else the text cursor.
    Right,
    /// ↑: the stop above.
    Up,
    /// ↓: the stop below.
    Down,
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

/// Where the keyboard is: the carousel is a focus stop above the two fields (the operator's
/// choice, Q212/Q213: ↑/↓ and tab/shift-tab move between the stops; on the carousel ←/→
/// switch the harness, in a field they move the text cursor).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stop {
    Carousel,
    Field(Field),
}

const STOPS: [Stop; 3] = [
    Stop::Carousel,
    Stop::Field(Field::Name),
    Stop::Field(Field::Cwd),
];

/// The modal's state.
#[derive(Clone, Debug)]
pub struct NewAgentForm {
    /// The selected harness (an index into [`HARNESSES`]).
    pub harness: usize,
    pub name: Text,
    pub cwd: Text,
    pub focus: Stop,
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
            focus: Stop::Field(Field::Name),
            status: None,
            busy: false,
            armed_at: now + ARM,
        }
    }

    /// The focused text field, if a field has the focus.
    fn field(&mut self) -> Option<&mut Text> {
        match self.focus {
            Stop::Field(Field::Name) => Some(&mut self.name),
            Stop::Field(Field::Cwd) => Some(&mut self.cwd),
            Stop::Carousel => None,
        }
    }

    /// The stop `d` from the focused one: tab goes round the three, the arrows stop at the
    /// ends.
    fn move_focus(&mut self, d: i32, round: bool) {
        let (i, n) = (
            STOPS.iter().position(|s| *s == self.focus).unwrap_or(1) as i32,
            STOPS.len() as i32,
        );
        let to = if round {
            (i + d).rem_euclid(n)
        } else {
            (i + d).clamp(0, n - 1)
        };
        self.focus = STOPS[to as usize];
    }

    /// A key at `now`.
    pub fn key(&mut self, k: FormKey, now: Instant) -> Outcome {
        if self.busy {
            return Outcome::Nothing; // the fields are disabled; Esc does not cancel a start
        }
        match k {
            FormKey::Esc => return Outcome::Cancel,
            FormKey::Submit => return self.submit(now),
            FormKey::Enter if self.focus == Stop::Field(Field::Cwd) => return self.submit(now),
            FormKey::Enter | FormKey::Tab => self.move_focus(1, true),
            FormKey::BackTab => self.move_focus(-1, true),
            FormKey::Down => self.move_focus(1, false),
            FormKey::Up => self.move_focus(-1, false),
            FormKey::Left | FormKey::Right if self.focus == Stop::Carousel => {
                self.harness = step(self.harness, if k == FormKey::Left { -1 } else { 1 });
            }
            FormKey::Char(c) if c.is_control() => return Outcome::Nothing,
            _ => {
                let Some(t) = self.field() else {
                    return Outcome::Nothing;
                }; // typing on the carousel
                match k {
                    FormKey::Char(c) => t.insert(c),
                    FormKey::Left => t.left(),
                    FormKey::Right => t.right(),
                    FormKey::Home => t.home(),
                    FormKey::End => t.end(),
                    FormKey::Backspace => t.back(),
                    FormKey::Delete => t.delete(),
                    _ => return Outcome::Nothing,
                }
            }
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
        let focus = match self.focus {
            Stop::Field(Field::Name) => Some((Field::Name, self.name.cursor)),
            Stop::Field(Field::Cwd) => Some((Field::Cwd, self.cwd.cursor)),
            Stop::Carousel => None,
        };
        Form {
            harness: self.harness,
            name: &self.name.value,
            slug: &view.slug,
            cwd: &self.cwd.value,
            focus: focus.filter(|_| !self.busy),
            status: self.status.as_deref(),
            carousel: self.focus == Stop::Carousel && !self.busy,
        }
    }
}

/// What the modal shows that the state derives.
pub struct FormView {
    pub slug: String,
}
