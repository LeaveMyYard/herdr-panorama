//! What the herdr side reports, applied to the view (Python's `_to_ui` targets and the core's
//! callbacks: `_seen`, `_filter_from_panel`, `focusing`, `herdr_focus`, `_tab_shown`,
//! `_panels_now`, `_open_key`).

use super::{Actor, End};
use crate::app::link::{Done, LinkEvent};
use crate::core::Event;
use crate::ui::input::{Host, Painted};
use panorama_core::view::Filter;
use std::io;
use std::time::Instant;

/// What the app does once it has ended (Python `after_exit`): the inbox popup opens only once
/// the graph popup is gone.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AfterExit {
    pub question: Option<i64>,
    pub pane: Option<String>,
}

impl Actor {
    /// One report from the herdr side; `Some` when the app ends.
    pub(super) fn on_link(&mut self, ev: LinkEvent) -> io::Result<Option<End>> {
        match ev {
            LinkEvent::Done(d) => self.on_done(d),
            LinkEvent::Core(e) => self.on_core(e),
        }
    }

    fn on_done(&mut self, d: Done) -> io::Result<Option<End>> {
        let now = Instant::now();
        match d {
            Done::Toast(t) => self.driver.toasts.push(t, now),
            Done::Exit => return Ok(Some(End::Quit)),
            Done::ExitThenInbox { question, pane } => {
                self.after_exit = Some(AfterExit { question, pane });
                return Ok(Some(End::Quit));
            }
            Done::Panels(map) => {
                let Some(link) = self.link.as_mut() else {
                    return Ok(None);
                };
                if link.published.panels != map {
                    link.published.panels = map;
                    self.publish();
                }
                return Ok(None);
            }
            Done::TabShown(tab) => {
                if self.link.as_mut().is_some_and(|l| l.tab_shown(tab)) {
                    self.publish();
                }
                return Ok(None);
            }
            Done::HerdrFocus(pane) => {
                let look = self.link.as_mut().and_then(|l| l.herdr_focus(&pane));
                return self.focused(look);
            }
            Done::FocusRead { announced, real } => {
                let look = self
                    .link
                    .as_mut()
                    .and_then(|l| l.focus_read(&announced, real));
                return self.focused(look);
            }
            Done::MyPane(_) => return Ok(None), // the link took its new id (`Link::next`)
            Done::NewAgentForm(_cwd) => {}      // the form in this pane: P4 PR 9b
        }
        self.repaint(Painted::Other).map(|()| None)
    }

    fn on_core(&mut self, e: Event) -> io::Result<Option<End>> {
        let now = Instant::now();
        match e {
            Event::Seen(ids) => {
                let out = self.driver.seen_in_a_panel(ids);
                if let Some(end) = self.act(out) {
                    return Ok(Some(end));
                }
            }
            Event::Filter(name) => {
                let Some(f) = Filter::from_name(&name) else {
                    return Ok(None);
                };
                if f != self.driver.app.view.filter {
                    let out = self.driver.app.set_filter(f, now);
                    if let Some(end) = self.act(out) {
                        return Ok(Some(end));
                    }
                }
                self.publish();
            }
            Event::Focusing(pane) => {
                let look = self.link.as_mut().and_then(|l| l.focusing(&pane));
                self.publish();
                return self.focused(look);
            }
            Event::Open { reply, .. } => {
                if let Some(link) = &self.link {
                    link.open(reply);
                }
                return Ok(None);
            }
            Event::Note(line) => {
                crate::app::panic::log(&self.log, &format!("core: {line}"));
                return Ok(None);
            }
        }
        self.repaint(Painted::Other).map(|()| None)
    }

    /// herdr's focus settled the view's look (`look`: whether it is focused, when known): the
    /// panels' looks follow at once.
    fn focused(&mut self, look: Option<bool>) -> io::Result<Option<End>> {
        self.publish();
        match look {
            Some(f) if f != self.driver.app.pane_focused => {
                self.driver.app.pane_focused = f;
                self.repaint(Painted::Other).map(|()| None)
            }
            _ => Ok(None),
        }
    }

    /// The whole state to the panels (nothing without a core).
    pub(super) fn publish(&mut self) {
        let app = &self.driver.app;
        if let Some(link) = self.link.as_mut() {
            link.publish(
                &app.graph.swarm,
                &app.graph.view.unseen,
                app.graph.view.filter,
            );
        }
    }

    /// A filter the operator chose here goes to every panel.
    pub(super) fn publish_filter(&mut self) {
        let f = self.driver.app.graph.view.filter;
        if self.link.as_ref().is_some_and(|l| l.filter_changed(f)) {
            self.publish();
        }
    }
}
