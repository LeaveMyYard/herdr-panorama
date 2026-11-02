//! Validate and persist a local association; never mutate herdr identity or lifecycle.
use super::{
    owner::{self, Owner},
    root,
    store::{Association, Store},
};
use crate::codex::Options;
use herdr_agent_kit::{
    agent_procs::ProcessTable,
    herdr_api::{Herdr, PaneId},
};
use std::time::Duration;

async fn pane_guard(herdr: &Herdr, pane: &PaneId, id: &str) -> Result<(), &'static str> {
    let info = tokio::time::timeout(Duration::from_millis(600), herdr.pane(pane))
        .await
        .map_err(|_| "pane lookup timed out")?
        .map_err(|_| "pane unavailable")?;
    if info.pane_id.as_ref() != Some(pane) || info.agent.as_deref() != Some("codex") {
        return Err("target pane is not the exact Codex pane requested");
    }
    if info
        .agent_session
        .as_ref()
        .and_then(|s| s.id())
        .is_some_and(|native| native != id)
    {
        return Err("pane has a different native Codex identity; refusing to override it");
    }
    Ok(())
}

pub(super) struct Request<'a> {
    pub pane: &'a str,
    pub id: &'a str,
    pub automatic: bool,
    pub owner: Option<Owner>,
}
pub(super) async fn bind(
    herdr: &Herdr,
    options: &Options,
    request: Request<'_>,
    store: &Store,
    table: &dyn Fn() -> Box<dyn ProcessTable>,
) -> Result<(), &'static str> {
    let Request {
        pane,
        id,
        automatic,
        owner,
    } = request;
    if !super::valid_thread_id(id) {
        return Err("thread ID must be a canonical lower-case UUID");
    }
    let target = PaneId::new(pane.to_owned());
    pane_guard(herdr, &target, id).await?;
    let embedded = owner.is_some();
    let owner = match owner {
        Some(owner) => {
            owner::check(herdr, &target, &owner, table().as_ref()).await?;
            owner
        }
        None => owner::foreground(&owner::info(herdr, &target).await?, pane, table().as_ref())
            .ok_or("cannot identify the target pane's Codex foreground process")?,
    };
    let existing = store.existing(pane);
    if automatic && existing.is_err() {
        return Err("invalid existing binding; use codex-bind THREAD_ID --pane PANE_ID");
    }
    if let Ok(Some(mut existing)) = existing {
        let conflict = existing.thread_id != id || existing.conflicted;
        if conflict || !automatic {
            existing.conflicted = true;
            store.write(&existing)?;
        }
        if automatic && conflict {
            return Err(
                "pane binding conflicts with another root; use codex-bind THREAD_ID --pane PANE_ID",
            );
        }
    }
    root::verify(options, id).await?;
    let mut association = Association {
        version: 1,
        pane_id: pane.into(),
        thread_id: id.into(),
        owner_pid: owner.pid,
        owner_start: owner.start.clone(),
        embedded,
        conflicted: true,
    };
    // Readers see unavailable until every fresh check succeeds. Failure leaves this
    // state intact instead of exposing an old root after an attempted switch.
    store.write(&association)?;
    pane_guard(herdr, &target, id).await?;
    if !owner::matches(
        &owner::info(herdr, &target).await?,
        pane,
        &owner,
        embedded,
        table().as_ref(),
    ) {
        return Err("pane ownership changed during binding; retry with --pane PANE_ID");
    }
    association.conflicted = false;
    store.write(&association)
}
