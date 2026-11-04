//! The lifetime lock outlives collection helpers; stop checks never need the home lock.
use super::{
    World,
    fs::{Home, Result},
    journal::Journal,
    liveness::{self, Idle, State},
    receipt::Receipt,
    serialize,
    triggers::Triggers,
};
use crate::{collect::paths::hooks_dir, host::Core};
use herdr_agent_kit::plugin_env::PluginEnv;
use std::{
    io::Write,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

pub fn watch(home: Home, r: Receipt, _lifetime: std::fs::File) -> Result<()> {
    let mut journal = Journal::open(&home, &r)?;
    let stop = Arc::new(AtomicBool::new(false));
    let monitor_stop = stop.clone();
    let monitor_path = home.path.clone();
    let monitor_id = r.id.clone();
    let monitor_sha = r.binary_sha.clone();
    let monitor = std::thread::spawn(move || {
        let opened = Home::open(&monitor_path);
        while !monitor_stop.load(Ordering::Relaxed) {
            let valid = opened.as_ref().ok().is_some_and(|h| {
                Receipt::read(h).is_ok_and(|r| {
                    r.id == monitor_id && r.binary_sha == monitor_sha && r.enabled(h).is_ok()
                })
            });
            if !valid {
                monitor_stop.store(true, Ordering::Relaxed);
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    });
    let result = (|| {
        let env = PluginEnv::from_env();
        let core = Core::default().with_stop(stop.clone());
        let mut source = crate::host::collect::app_source(env.clone(), core.clone());
        let mut triggers = Triggers::new(
            core,
            vec![
                hooks_dir(&env),
                env.xdg_state_home().join("herdr-panorama/codex-bindings"),
            ],
        );
        // Test-only startup latency; release builds have no such knob.
        #[cfg(debug_assertions)]
        if let Some(ms) = std::env::var("PANORAMA_TEST_READY_DELAY_MS")
            .ok()
            .and_then(|s| s.parse().ok())
        {
            std::thread::sleep(Duration::from_millis(ms));
        }
        // A parent that stopped waiting has closed the pipe; that is not a writer failure.
        let mut out = std::io::stdout();
        let _ = writeln!(out, "READY {}", r.id).and_then(|()| out.flush());
        let start = Instant::now();
        let mut idle = Idle::default();
        let mut app = State::Unknown;
        let mut next_probe = Instant::now();
        let mut next_collect = Instant::now();
        let mut last_collect = Instant::now() - Duration::from_secs(5);
        let mut last: Option<World> = None;
        let mut dirty_pending = true;
        while !stop.load(Ordering::Relaxed) {
            if Instant::now() >= next_probe {
                let previous = app;
                app = liveness::probe(&home.path, &stop);
                if app == State::Running && previous != State::Running {
                    next_collect = Instant::now();
                }
                next_probe = Instant::now() + Duration::from_secs(2);
                if idle.observe(app, start.elapsed()) {
                    break;
                }
                if app != State::Running {
                    triggers.pause();
                }
                if app == State::Unknown {
                    let mut degraded = last.clone().unwrap_or_else(|| World {
                        swarm: panorama_core::model::Swarm {
                            generated_at: crate::snapshot::now(),
                            herdr_ok: false,
                            inbox_ok: false,
                            ..Default::default()
                        },
                        roots: Vec::new(),
                    });
                    degraded
                        .swarm
                        .errors
                        .push("herdr-app status unavailable; last collection retained".into());
                    if !stop.load(Ordering::Relaxed) {
                        journal.publish(&home, &r, &serialize::bytes(&degraded)?)?;
                    }
                }
            }
            if app == State::Running {
                dirty_pending |= triggers.changed();
                if (dirty_pending && last_collect.elapsed() >= Duration::from_millis(500))
                    || Instant::now() >= next_collect
                {
                    dirty_pending = false;
                    let collected = source();
                    let pending = collected.pending;
                    let mut world = World::from(collected);
                    if stop.load(Ordering::Relaxed) {
                        break;
                    }
                    if triggers.event_error {
                        world
                            .swarm
                            .errors
                            .push("herdr-core events unavailable; polling".into());
                    }
                    if !world.swarm.inbox_ok {
                        world.swarm.errors.push("inbox source unavailable".into());
                    }
                    journal.publish(&home, &r, &serialize::bytes(&world)?)?;
                    triggers.observe(&world);
                    last = Some(world);
                    last_collect = Instant::now();
                    next_collect = Instant::now()
                        + if pending {
                            Duration::from_millis(100)
                        } else {
                            Duration::from_secs(5)
                        };
                }
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        // Source/observer and event helpers drop and reap before lifetime ownership is released.
        drop(triggers);
        drop(source);
        Ok(())
    })();
    stop.store(true, Ordering::Relaxed);
    let _ = monitor.join();
    result
}
