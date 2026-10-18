//! The collector end to end over the captured demo scenario (Python `test_collect.py`):
//! herdr's snapshot, the transcripts, a workflow, the explicit channel and the inbox.

mod collect_support;

use collect_support::{S1, STALE, Scenario};
use panorama::collect::Collector;
use panorama_core::model::{Agent, Orchestrator, Status, Swarm, ThreadSource, ThreadStatus};
use serde_json::Value;
use std::io::Write;
use std::path::PathBuf;

const H: f64 = 3600.0;

fn collect(sc: &Scenario, c: &mut Collector, now: f64, window: f64, inbox: bool) -> Swarm {
    let qs = sc.questions();
    c.collect(
        Some(sc.snapshot()),
        inbox.then_some(qs.as_slice()),
        now,
        window,
        STALE,
    )
}

fn demo(now_offset: f64, window: f64) -> Swarm {
    let sc = Scenario::new("demo");
    collect(
        &sc,
        &mut sc.collector(),
        sc.now() + now_offset,
        window,
        true,
    )
}

fn by_label<'a>(sw: &'a Swarm, label: &str) -> &'a Orchestrator {
    sw.orchestrators
        .iter()
        .find(|o| o.tab_label == label)
        .unwrap_or_else(|| panic!("no {label}"))
}

fn agents(o: &Orchestrator) -> Vec<&Agent> {
    o.agents().collect()
}

/// The demo's first session transcript.
fn s1_root(sc: &Scenario) -> PathBuf {
    sc.projects()
        .join("-home-user-project")
        .join(format!("{S1}.jsonl"))
}

fn append(path: &PathBuf, text: &str) {
    std::fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(path)
        .unwrap()
        .write_all(text.as_bytes())
        .unwrap();
}

#[test]
fn demo_swarm_shape() {
    let sw = demo(0.0, 6.0 * H);
    let labels: Vec<_> = sw
        .orchestrators
        .iter()
        .map(|o| o.tab_label.as_str())
        .collect();
    assert_eq!(
        labels,
        [
            "Billing API",
            "Search refactor",
            "Docs site",
            "Release notes"
        ]
    ); // the shell pane is skipped
    let billing = by_label(&sw, "Billing API");
    assert_eq!(billing.inbox, [1, 2]);
    assert_eq!(billing.flavour, "pr-review");
    let thread = |name: &str| billing.threads.iter().find(|t| t.name == name).unwrap();
    let t42 = thread("#42");
    let status: Vec<(String, Status)> = t42
        .agents
        .iter()
        .map(|a| (a.id[..8].to_owned(), a.status.clone()))
        .collect();
    for (id, st) in [
        ("a1author", Status::Done),
        ("a4review", Status::Reviewing),
        ("a2review", Status::Done),
        ("a3explor", Status::Done),
    ] {
        assert!(
            status.contains(&(id.to_owned(), st.clone())),
            "{id} {st:?} in {status:?}"
        );
    }
    assert_eq!(status.len(), 4);
    let nested = t42.agents.iter().find(|a| a.id.starts_with("a3")).unwrap();
    assert_eq!(
        (nested.parent.as_deref(), nested.depth),
        (Some("a1author0000000001"), 2)
    );
    let phases: Vec<_> = t42.phases.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(phases, ["author", "review r1", "author 2", "review r2"]);
    let a1 = t42
        .agents
        .iter()
        .find(|a| a.id.starts_with("a1author"))
        .unwrap();
    assert_eq!((a1.resumes, a1.tool_calls), (1, 44)); // the notice's count wins over the file's
    let t44 = thread("#44");
    assert!(
        t44.status == ThreadStatus::Failed && t44.agents.iter().any(|a| a.status == Status::Failed)
    );
    let draft: Vec<_> = thread("Draft retry banner copy")
        .agents
        .iter()
        .map(|a| a.status.clone())
        .collect();
    assert_eq!(draft, [Status::Waiting]);
    assert_eq!(thread("Billing API").source, ThreadSource::Session); // Q1 is no agent's
    let wf = &by_label(&sw, "Search refactor").threads[0];
    assert_eq!(
        (wf.source.clone(), wf.name.as_str()),
        (ThreadSource::Workflow, "search-index-rebuild")
    );
    let wf_phases: Vec<_> = wf
        .phases
        .iter()
        .map(|p| (p.name.as_str(), p.state.code()))
        .collect();
    assert_eq!(wf_phases, [("Plan", "done"), ("Build", "now")]);
    let docs = by_label(&sw, "Docs site");
    assert_eq!(
        docs.pending,
        ["Publish the v2 docs now or after the release?"]
    );
    let explicit = docs
        .threads
        .iter()
        .find(|t| t.source == ThreadSource::Explicit)
        .unwrap();
    assert_eq!(
        (
            explicit.name.as_str(),
            explicit.agents[0].role.as_str(),
            explicit.pct
        ),
        ("Docs v2 release", "writer", Some(60))
    );
    assert_eq!(
        by_label(&sw, "Release notes").threads[0].status,
        ThreadStatus::Done
    );
}

#[test]
fn done_window_hides_old_threads() {
    let sw = demo(0.0, 60.0);
    let notes = by_label(&sw, "Release notes");
    assert!(notes.threads.is_empty() && notes.hidden_done == 0); // its agent is outside the window entirely
}

#[test]
fn later_ticks_read_only_what_was_appended() {
    let sc = Scenario::new("demo");
    let mut c = sc.collector();
    let first = collect(&sc, &mut c, sc.now(), 6.0 * H, true);
    // the transcript rewritten in place at the same size and time: a collector that read it
    // again would see the new tool count in the agent's notice; one that reads only appended
    // bytes sees nothing new
    let root = s1_root(&sc);
    let (meta, text) = (
        std::fs::metadata(&root).unwrap(),
        std::fs::read_to_string(&root).unwrap(),
    );
    let changed = text.replace("<tool_uses>44</tool_uses>", "<tool_uses>45</tool_uses>");
    assert_ne!(changed, text);
    std::fs::write(&root, &changed).unwrap();
    std::fs::File::options()
        .write(true)
        .open(&root)
        .unwrap()
        .set_modified(meta.modified().unwrap())
        .unwrap();
    let second = collect(&sc, &mut c, sc.now(), 6.0 * H, true);
    assert_eq!(second, first);
    assert_eq!(second.orchestrators[0].tab_label, "Billing API");
}

#[test]
fn lost_agents_end_silently() {
    let sw = demo(7.0 * H, 24.0 * H);
    let ags: Vec<&Agent> = sw
        .orchestrators
        .iter()
        .flat_map(agents)
        .filter(|a| !a.id.starts_with("main:"))
        .collect();
    let ended = [
        Status::Done,
        Status::Cancelled,
        Status::Failed,
        Status::Inferred,
    ];
    assert!(
        ags.iter().all(|a| ended.contains(&a.status)),
        "{:?}",
        ags.iter().map(|a| &a.status).collect::<Vec<_>>()
    );
    assert!(
        ags.iter()
            .any(|a| a.status == Status::Inferred && a.inferred)
    ); // issue #2: never a plain done
}

#[test]
fn inbox_tab_prefix_is_stripped() {
    let mut sc = Scenario::new("demo");
    sc.input["snapshot"]["tabs"][0]["label"] = Value::from("❓2 Billing API");
    let sw = sc
        .collector()
        .collect(Some(sc.snapshot()), None, sc.now(), 6.0 * H, STALE);
    assert_eq!(sw.orchestrators[0].tab_label, "Billing API");
}

#[test]
fn a_session_continued_in_a_new_transcript_is_followed() {
    let base = demo(0.0, 6.0 * H);
    let want = by_label(&base, "Billing API");
    let sc = Scenario::new("demo");
    let new = "55555555-5555-4555-8555-555555555555";
    let old_root = s1_root(&sc);
    let new_root = old_root.with_file_name(format!("{new}.jsonl"));
    std::fs::rename(&old_root, &new_root).unwrap();
    let (old_dir, new_dir) = (old_root.with_extension(""), new_root.with_extension(""));
    if old_dir.is_dir() {
        std::fs::rename(&old_dir, &new_dir).unwrap();
    }
    // what is left of the old session: its end, pointing on (byte for byte as Claude Code writes it)
    std::fs::write(
        &old_root,
        format!(
            "{{\"type\":\"ai-title\",\"aiTitle\":\"Billing API\",\"sessionId\":\"{S1}\"}}\n\
             {{\"type\":\"continued-in\",\"timestamp\":\"2026-10-16T18:41:47.545Z\",\"sessionId\":\"{S1}\",\"continuedInSessionId\":\"{new}\"}}\n"
        ),
    )
    .unwrap();
    let sw = collect(&sc, &mut sc.collector(), sc.now(), 6.0 * H, true); // herdr still says S1
    let got = by_label(&sw, "Billing API");
    assert_eq!(got.session_id.as_deref(), Some(new));
    assert_eq!(got.transcript.as_deref(), Some(new_root.to_str().unwrap()));
    let names = |o: &Orchestrator| o.threads.iter().map(|t| t.name.clone()).collect::<Vec<_>>();
    assert_eq!(names(got), names(want));
    assert!(agents(got).len() == agents(want).len() && agents(got).len() > 1);
    assert_eq!(got.inbox, [1, 2]); // questions asked under the old id still belong to it
}

#[test]
fn a_continuation_loop_is_not_followed_forever() {
    let sc = Scenario::new("demo");
    let root = s1_root(&sc);
    let other = "66666666-6666-4666-8666-666666666666";
    append(
        &root,
        &format!(
            "{{\"type\":\"continued-in\",\"sessionId\":\"{S1}\",\"continuedInSessionId\":\"{other}\"}}\n"
        ),
    );
    append(
        &root.with_file_name(format!("{other}.jsonl")),
        &format!(
            "{{\"type\":\"continued-in\",\"sessionId\":\"{other}\",\"continuedInSessionId\":\"{S1}\"}}\n"
        ),
    );
    let sw = collect(&sc, &mut sc.collector(), sc.now(), 6.0 * H, false);
    assert!(
        sw.orchestrators
            .iter()
            .any(|o| o.tab_label == "Billing API")
    );
}

#[test]
fn a_resumed_old_session_is_no_longer_followed() {
    let sc = Scenario::new("demo");
    let root = s1_root(&sc);
    let other = "66666666-6666-4666-8666-666666666666";
    append(
        &root.with_file_name(format!("{other}.jsonl")),
        &format!("{{\"type\":\"ai-title\",\"aiTitle\":\"elsewhere\",\"sessionId\":\"{other}\"}}\n"),
    );
    append(
        &root,
        &format!(
            "{{\"type\":\"continued-in\",\"timestamp\":\"2026-10-16T18:41:47.545Z\",\"sessionId\":\"{S1}\",\"continuedInSessionId\":\"{other}\"}}\n\
             {{\"type\":\"last-prompt\",\"lastPrompt\":\"x\",\"sessionId\":\"{S1}\"}}\n\
             {{\"type\":\"cost-state\",\"sessionId\":\"{S1}\",\"totalCostUSD\":1.0}}\n"
        ),
    );
    let mut c = sc.collector();
    let billing = |sw: &Swarm| {
        sw.orchestrators
            .iter()
            .find(|o| o.pane_id == "w1:p1")
            .unwrap()
            .session_id
            .clone()
    };
    assert_eq!(
        billing(&collect(&sc, &mut c, sc.now(), 6.0 * H, false)).as_deref(),
        Some(other)
    );
    // `claude --resume <old>`: the conversation goes on in the old file
    append(
        &root,
        &format!(
            "{{\"parentUuid\":null,\"isSidechain\":false,\"type\":\"user\",\"message\":{{\"role\":\"user\",\"content\":[{{\"type\":\"text\",\"text\":\"go on\"}}]}},\"uuid\":\"u1\",\"timestamp\":\"2026-10-16T19:00:00.000Z\",\"sessionId\":\"{S1}\"}}\n"
        ),
    );
    assert_eq!(
        billing(&collect(&sc, &mut c, sc.now(), 6.0 * H, false)).as_deref(),
        Some(S1)
    );
}

#[test]
fn a_continuation_that_is_not_a_session_id_is_ignored() {
    for bad in [
        "../../outside",
        "../x",
        "*",
        "/etc/passwd",
        "/tmp/elsewhere/x",
    ] {
        let sc = Scenario::new("demo");
        let outside = sc.root().join("outside.jsonl");
        std::fs::write(
            &outside,
            "{\"type\":\"ai-title\",\"aiTitle\":\"not a session\",\"sessionId\":\"x\"}\n",
        )
        .unwrap();
        let rec = serde_json::json!({"type": "continued-in", "sessionId": S1, "continuedInSessionId": bad});
        append(&s1_root(&sc), &format!("{rec}\n"));
        let mut c = sc.collector();
        let sw = collect(&sc, &mut c, sc.now(), 6.0 * H, false);
        let billing = sw
            .orchestrators
            .iter()
            .find(|o| o.pane_id == "w1:p1")
            .unwrap();
        assert_eq!(billing.session_id.as_deref(), Some(S1), "{bad}");
        assert!(c.continuations(&[S1.to_owned()]).is_empty(), "{bad}");
        assert!(outside.is_file());
    }
}

#[test]
fn a_question_without_a_session_belongs_to_its_pane() {
    use herdr_agent_kit::internal::inbox_contract::sqlite::OpenQuestion;
    let sc = Scenario::new("demo");
    let q = |id: i64, session: Option<&str>, pane: Option<&str>| OpenQuestion {
        id,
        session_id: session.map(str::to_owned),
        pane_id: pane.map(str::to_owned),
        asked_by: None,
        question: format!("Q{id}?"),
        created_at: None,
    };
    let qs = [
        q(7, None, Some("w1:p2")),
        q(8, Some(""), Some("w1:p2")),
        q(9, Some(S1), Some("w1:p2")),
    ];
    let sw = sc
        .collector()
        .collect(Some(sc.snapshot()), Some(&qs), sc.now(), 6.0 * H, STALE);
    let search = sw
        .orchestrators
        .iter()
        .find(|o| o.pane_id == "w1:p2")
        .unwrap();
    assert_eq!(search.inbox, [7, 8]); // a session id, when there is one, decides
    assert_eq!(search.inbox_texts, ["Q7?", "Q8?"]);
    assert_eq!(by_label(&sw, "Billing API").inbox, [9]);
}

#[test]
fn a_continuation_chain_is_followed_at_most_eight_hops() {
    let sc = Scenario::new("demo");
    let root = s1_root(&sc);
    let ids: Vec<String> = (1..=10)
        .map(|i| format!("{i:08}-0000-4000-8000-000000000000"))
        .collect();
    let line = |from: &str, to: &str| {
        format!(
            "{{\"type\":\"continued-in\",\"sessionId\":\"{from}\",\"continuedInSessionId\":\"{to}\"}}\n"
        )
    };
    append(&root, &line(S1, &ids[0]));
    for w in ids.windows(2) {
        append(
            &root.with_file_name(format!("{}.jsonl", w[0])),
            &line(&w[0], &w[1]),
        );
    }
    append(
        &root.with_file_name(format!("{}.jsonl", ids[9])),
        "{\"type\":\"ai-title\",\"aiTitle\":\"x\"}\n",
    );
    let sw = collect(&sc, &mut sc.collector(), sc.now(), 6.0 * H, false);
    let billing = sw
        .orchestrators
        .iter()
        .find(|o| o.pane_id == "w1:p1")
        .unwrap();
    assert_eq!(billing.session_id.as_deref(), Some(ids[7].as_str())); // S1 + 8 hops
}
