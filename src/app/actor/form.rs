//! The + New agent modal's steps carried out: a start goes to the herdr side, the rest
//! repaints.

use super::{Actor, End};
use crate::app::form::FormStep;
use crate::ui::input::Painted;
use std::io;

impl Actor {
    pub(super) fn on_form(&mut self, step: FormStep) -> io::Result<Option<End>> {
        match step {
            FormStep::Nothing => return Ok(None),
            FormStep::Start { name, cwd, kind } => match &self.link {
                Some(l) => l.start_agent(name, cwd, kind),
                None => self.driver.form = None, // no herdr side (a test): nothing to start
            },
            FormStep::Repaint | FormStep::Closed => {}
        }
        self.repaint(Painted::Other).map(|()| None)
    }
}
