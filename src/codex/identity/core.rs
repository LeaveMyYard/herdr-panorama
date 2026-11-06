//! Core address adapter for the existing private binding, root and owner proofs.
use super::{
    owner::{self, Owner},
    root,
    store::{self, Association, Store},
};
use crate::{
    codex::Options,
    host::{
        Core,
        context::{pane_key, valid_address},
    },
};
use herdr_agent_kit::{agent_procs::ProcessTable, herdr_api::ProcessInfo, plugin_env::PluginEnv};
use serde_json::Value;

pub fn resolve(
    env: &PluginEnv,
    address: &str,
    process: &Value,
    table: &dyn ProcessTable,
) -> Option<String> {
    if address.contains('/') {
        return None;
    }
    let a = store::core_existing(env, address)?;
    let info: ProcessInfo = serde_json::from_value(process["process_info"].clone()).ok()?;
    (!a.conflicted
        && crate::host::records::namespace_matches(a.pidns.as_deref())
        && owner::matches(
            &info,
            &pane_key(address),
            &Owner {
                pid: a.owner_pid,
                start: a.owner_start,
            },
            a.embedded,
            table,
        ))
    .then_some(a.thread_id)
}

fn info(core: &Core, address: &str) -> Result<ProcessInfo, &'static str> {
    let value = core
        .process(address)
        .ok_or("core foreground ownership unavailable")?;
    serde_json::from_value(value["process_info"].clone())
        .map_err(|_| "invalid core foreground ownership")
}

pub(crate) fn owns_record(
    info: &Value,
    address: &str,
    pid: u32,
    start: &str,
    table: &dyn ProcessTable,
) -> bool {
    let parsed: Result<ProcessInfo, _> = serde_json::from_value(info["process_info"].clone());
    parsed.is_ok_and(|p| {
        owner::matches(
            &p,
            &pane_key(address),
            &Owner {
                pid,
                start: start.into(),
            },
            false,
            table,
        )
    })
}

pub(super) async fn bind(
    core: &Core,
    env: &PluginEnv,
    options: &Options,
    address: &str,
    request: super::bind::Request<'_>,
    table: &dyn Fn() -> Box<dyn ProcessTable>,
) -> Result<(), &'static str> {
    if !valid_address(address) || address.contains('/') {
        return Err("local exact core terminal required");
    }
    if !super::valid_thread_id(request.id) {
        return Err("invalid thread ID");
    }
    let pane = pane_key(address);
    let store = Store::core_lock(env, address)?;
    let embedded = request.owner.is_some();
    let current = info(core, address)?;
    let owner = request
        .owner
        .or_else(|| owner::foreground(&current, &pane, table().as_ref()))
        .ok_or("cannot identify core Codex foreground process")?;
    if !owner::matches(&current, &pane, &owner, embedded, table().as_ref()) {
        return Err("core hook does not own the exact foreground process");
    }
    let existing = store.existing(&pane);
    if request.automatic && existing.is_err() {
        return Err("invalid existing core binding; use --terminal");
    }
    if let Ok(Some(mut a)) = existing {
        let conflict = a.thread_id != request.id || a.conflicted;
        if conflict || !request.automatic {
            a.conflicted = true;
            store.write(&a)?;
        }
        if request.automatic && conflict {
            return Err("core binding conflicts; use codex-bind THREAD_ID --terminal ADDRESS");
        }
    }
    root::verify(options, request.id).await?;
    let mut a = Association {
        version: 1,
        pane_id: pane.clone(),
        thread_id: request.id.into(),
        owner_pid: owner.pid,
        owner_start: owner.start.clone(),
        embedded,
        conflicted: true,
        host: Some("herdr-core".into()),
        terminal: Some(address.into()),
        pidns: herdr_agent_kit::agent_procs::own_pidns(),
    };
    store.write(&a)?;
    if !owner::matches(
        &info(core, address)?,
        &pane,
        &owner,
        embedded,
        table().as_ref(),
    ) {
        return Err("core foreground changed during binding");
    }
    a.conflicted = false;
    store.write(&a)?;
    // A terminal-aware lifecycle record also lets a plain-shell snapshot select its host.
    let mut cx = crate::hook::Ctx::from_env(env);
    cx.host.host = crate::host::Host::Core;
    cx.host.terminal = Some(address.into());
    cx.harness = "codex";
    cx.identity = Box::new(move || {
        Some((
            owner.pid,
            owner.start.clone(),
            herdr_agent_kit::agent_procs::own_pidns(),
        ))
    });
    let _ = crate::hook::record::append(
        request.id,
        vec![
            ("v".into(), crate::snapshot::pyjson::J::Int(1)),
            ("event".into(), crate::snapshot::pyjson::J::str("session")),
            ("ts".into(), crate::snapshot::pyjson::J::Float(cx.now)),
        ],
        &cx,
    );
    Ok(())
}

#[cfg(test)]
mod tests;
