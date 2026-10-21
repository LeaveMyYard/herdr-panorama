//! `panorama new-agent` (Python `tui.NewAgentApp`): the + New agent modal alone, in its own
//! herdr popup (opened from a pane too narrow for it). It says it started (the opener waits
//! for that mark after a request herdr never answered), and closes when the agent runs or
//! the form is cancelled.

use super::form::{NewAgentForm, Outcome};
use super::keys::form_key;
use super::overlay::SHADE;
use super::real::{RealStart, START_TIMEOUT_MS};
use super::start::new_agent;
use crate::home::config::per_tab;
use crate::panel::act::new_agent::marker;
use crate::panels::fit;
use crate::panels::real::RealOps;
use crate::settings::Settings;
use crate::ui::term;
use crossterm::event::{Event, EventStream, MouseButton, MouseEventKind};
use futures_util::StreamExt;
use herdr_agent_kit::internal::plugin_paths::PANORAMA_ID;
use herdr_agent_kit::internal::plugin_paths::panorama::{session_key, state_dir};
use herdr_agent_kit::plugin_env::PluginEnv;
use panorama_core::view::Screen;
use panorama_core::view::new_agent::{Modal, render};
use std::path::Path;
use std::time::Instant;

/// Runs the popup until the agent runs or the form is cancelled; the exit code.
pub fn run() -> i32 {
    let env = PluginEnv::from_env();
    mark_started(&marker(&state_dir(&env), &session_key(&env)));
    let result = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .and_then(|rt| rt.block_on(serve(&env)));
    term::restore();
    i32::from(result.is_err())
}

/// The popup's process is up: the time, for the opener to compare with its request's.
pub fn mark_started(path: &Path) {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0.0, |d| d.as_secs_f64());
    if let Some(d) = path.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    let _ = std::fs::write(path, now.to_string());
}

/// The form's directory: `PANORAMA_NEW_AGENT_CWD`, else the home directory.
pub fn cwd(env: &PluginEnv) -> String {
    env.var("PANORAMA_NEW_AGENT_CWD")
        .map(str::to_owned)
        .or_else(|| env.home().map(|h| h.to_string_lossy().into_owned()))
        .unwrap_or_else(|| "~".into())
}

/// The popup's picture: the 1C modal on the popup's own background.
pub fn picture(form: &NewAgentForm, w: i32, h: i32) -> (Screen, Modal) {
    let view = form.view();
    let modal = render(&form.form(&view), w, h);
    let mut s = Screen::new(w, h);
    s.fill_bg(0, 0, w, h, SHADE);
    crate::new_agent::overlay::place(&mut s, &modal.screen);
    (s, modal)
}

async fn serve(env: &PluginEnv) -> std::io::Result<()> {
    let herdr = crate::trace::client();
    let mut settings = Settings::from_env(env);
    let table = settings.panorama_table();
    let ops = RealStart {
        herdr: herdr.clone(),
        plugin_env: crate::new_agent::real::plugin_env(env),
        root: env.plugin_root().map(|r| r.to_string_lossy().into_owned()),
        panels: RealOps {
            herdr,
            plugin_id: env.plugin_id(PANORAMA_ID).to_owned(),
            root: env.plugin_root().map(Path::to_path_buf),
            width: fit::width(table.get("cockpit_width")),
        },
        start_timeout_ms: START_TIMEOUT_MS,
    };
    let per_tab = per_tab(&table);
    let mut form = NewAgentForm::new(&cwd(env), Instant::now());
    let mut t = term::setup()?;
    let mut input = EventStream::new();
    type Start<'a> =
        std::pin::Pin<Box<dyn std::future::Future<Output = Result<String, String>> + 'a>>;
    let mut started: Option<Start<'_>> = None;
    loop {
        let size = t.size()?;
        let (screen, modal) = picture(&form, i32::from(size.width), i32::from(size.height));
        t.draw(|f| term::copy(&screen, f.buffer_mut()))?;
        let out = tokio::select! {
            r = async { started.as_mut().expect("a start").await }, if started.is_some() => {
                started = None;
                if form.finished(r).is_some() {
                    return Ok(()); // it runs: the popup closes over its new tab
                }
                continue;
            }
            ev = input.next() => match ev {
                Some(Ok(Event::Key(k))) => match form_key(k) {
                    Some(k) => form.key(k, Instant::now()),
                    None => continue,
                },
                Some(Ok(Event::Mouse(m))) if m.kind == MouseEventKind::Down(MouseButton::Left) => {
                    form.click(i32::from(m.column), i32::from(m.row), &modal)
                }
                Some(Ok(_)) => continue,
                Some(Err(e)) => return Err(e),
                None => return Ok(()),
            },
        };
        match out {
            Outcome::Cancel => return Ok(()),
            Outcome::Start { name, cwd, kind } => {
                let ops = &ops;
                started = Some(Box::pin(async move {
                    new_agent(ops, &name, cwd.as_deref(), kind, per_tab).await
                }));
            }
            Outcome::Changed | Outcome::Nothing => {}
        }
    }
}
