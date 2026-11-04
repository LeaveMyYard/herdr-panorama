//! The collector end to end over the captured demo scenario (Python `test_collect.py`):
//! herdr's snapshot, the transcripts, a workflow, the explicit channel and the inbox.

mod collect_support;

use collect_support::{S1, STALE, Scenario};
use panorama::collect::Collector;
use panorama_core::model::{Agent, Root, Status, Swarm};
use panorama_core::view::orch_state::tree_done;
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

fn by_label<'a>(sw: &'a Swarm, label: &str) -> &'a Root {
    sw.roots
        .iter()
        .find(|o| o.tab_label == label)
        .unwrap_or_else(|| panic!("no {label}"))
}

fn agents(o: &Root) -> Vec<&Agent> {
    o.agents.iter().collect()
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
    let labels: Vec<_> = sw.roots.iter().map(|o| o.tab_label.as_str()).collect();
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
    // one uniform tree per root
    for o in &sw.roots {
        // no agent stands for the session: its own state is on the root
        assert!(o.agents.iter().all(|a| !a.id.starts_with("main:")));
    }
    let status: Vec<(String, Status)> = billing
        .agents
        .iter()
        .map(|a| (a.id[..8].to_owned(), a.status.clone()))
        .collect();
    for (id, st) in [
        ("a1author", Status::Done),
        ("a4review", Status::Reviewing),
        ("a2review", Status::Done),
        ("a3explor", Status::Done),
        ("b3fixer0", Status::Failed),
        ("c1banner", Status::Waiting),
    ] {
        assert!(
            status.contains(&(id.to_owned(), st.clone())),
            "{id} {st:?} in {status:?}"
        );
    }
    assert_eq!(status.len(), 7);
    let nested = billing
        .agents
        .iter()
        .find(|a| a.id.starts_with("a3"))
        .unwrap();
    assert_eq!(
        (nested.parent.as_deref(), nested.depth),
        (Some("a1author0000000001"), 2)
    );
    let a1 = billing
        .agents
        .iter()
        .find(|a| a.id.starts_with("a1author"))
        .unwrap();
    assert_eq!((a1.resumes, a1.tool_calls), (1, 44)); // the notice's count wins over the file's
    assert!(!tree_done(billing));
    // Q1 is no agent's: it stays the root's own question
    assert!(billing.agents.iter().all(|a| !a.question_ids.contains(&1)));
    // a Workflow run's agents are the root's children, its label their role (D8)
    let search = by_label(&sw, "Search refactor");
    let roles: Vec<(&str, Option<&str>, &str)> = search
        .agents
        .iter()
        .map(|a| (&a.id[..8], a.parent.as_deref(), a.role.as_str()))
        .collect();
    assert_eq!(
        roles,
        [
            ("w2build0", None, "author"),
            ("w3build0", None, "author"),
            ("w1plan00", None, "planner")
        ]
    );
    let docs = by_label(&sw, "Docs site");
    assert_eq!(
        docs.pending,
        ["Publish the v2 docs now or after the release?"]
    );
    // the explicit channel is no longer read: its role and progress are gone
    let guide: Vec<&str> = docs.agents.iter().map(|a| a.role.as_str()).collect();
    assert_eq!(guide, ["author"]);
    assert!(tree_done(by_label(&sw, "Release notes")));
}

#[test]
fn done_window_hides_old_finished_agents_and_counts_them() {
    let wide = demo(0.0, 1e9);
    // the failed fixer ended a minute before now
    let sw = demo(0.0, 30.0);
    let notes = by_label(&sw, "Release notes");
    // its agent is outside the window: the root stays, its tree empty
    assert!(notes.agents.is_empty());
    assert_eq!(notes.hidden_finished, 1);
    assert_eq!(by_label(&wide, "Release notes").hidden_finished, 0);
    // a failed agent ages out like a done one (D4); a live one stays however old
    let billing = |sw: &Swarm| -> Vec<(String, Status)> {
        by_label(sw, "Billing API")
            .agents
            .iter()
            .map(|a| (a.id[..8].to_owned(), a.status.clone()))
            .collect()
    };
    let (all, shown) = (billing(&wide), billing(&sw));
    assert!(
        all.contains(&("b3fixer0".into(), Status::Failed)),
        "{all:?}"
    );
    assert!(
        shown
            .iter()
            .all(|(_, st)| st.is_live() && *st != Status::Failed),
        "{shown:?}"
    );
    assert!(!shown.is_empty());
    assert_eq!(
        by_label(&sw, "Billing API").hidden_finished as usize,
        all.len() - shown.len()
    );
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
    assert_eq!(second.roots[0].tab_label, "Billing API");
}

#[test]
fn lost_agents_end_silently() {
    let sw = demo(7.0 * H, 24.0 * H);
    let ags: Vec<&Agent> = sw
        .roots
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
    assert_eq!(sw.roots[0].tab_label, "Billing API");
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
    assert_eq!(got.name(), want.name());
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
    let billing = sw.roots.iter().find(|o| o.pane_id == "w1:p1").unwrap();
    assert_eq!(billing.tab_label, "Billing API");
    assert_eq!(billing.session_id.as_deref(), Some(other)); // the first repeat ends the walk
    assert!(
        billing
            .transcript
            .as_deref()
            .unwrap()
            .ends_with(&format!("{other}.jsonl"))
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
        sw.roots
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
        let billing = sw.roots.iter().find(|o| o.pane_id == "w1:p1").unwrap();
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
    let search = sw.roots.iter().find(|o| o.pane_id == "w1:p2").unwrap();
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
    let billing = sw.roots.iter().find(|o| o.pane_id == "w1:p1").unwrap();
    assert_eq!(billing.session_id.as_deref(), Some(ids[7].as_str())); // S1 + 8 hops
}

#[test]
fn a_non_ascii_model_in_a_meta_file_is_read_as_python_reads_it() {
    let sc = Scenario::new("demo");
    let meta = sc
        .projects()
        .join("-home-user-project")
        .join(S1)
        .join("subagents/agent-a3explore000000001.meta.json");
    let text = std::fs::read_to_string(&meta).unwrap().replace(
        "\"model\": \"haiku\"",
        "\"model\": \"claude-\u{f6}-opus-4\"",
    );
    std::fs::write(&meta, text).unwrap();
    let sw = collect(&sc, &mut sc.collector(), sc.now(), 6.0 * H, true);
    assert!(sw.errors.is_empty(), "{:?}", sw.errors);
    let a3 = sw
        .roots
        .iter()
        .flat_map(agents)
        .find(|a| a.id.starts_with("a3explore"))
        .unwrap();
    assert_eq!(a3.model.as_deref(), Some("opus 4"));
}

#[test]
fn roots_and_agents_carry_their_titles() {
    let sw = demo(0.0, 6.0 * H);
    let billing = by_label(&sw, "Billing API");
    assert_eq!(
        (billing.title.as_str(), billing.title_source.as_str()),
        ("Billing API", "tab")
    );
    let author = agents(billing)
        .into_iter()
        .find(|a| a.id.starts_with("a1author"))
        .unwrap();
    assert_eq!(
        (author.title.as_str(), author.title_source.as_str()),
        ("Author PR #42 invoice export", "spawner")
    );
}

/// The demo's a3explore agent after `edit` changed its files.
fn explore_after(edit: impl FnOnce(&Scenario)) -> Agent {
    let sc = Scenario::new("demo");
    edit(&sc);
    let sw = collect(&sc, &mut sc.collector(), sc.now(), 6.0 * H, true);
    sw.roots
        .iter()
        .flat_map(agents)
        .find(|a| a.id.starts_with("a3explore"))
        .unwrap()
        .clone()
}

fn replace_in(path: PathBuf, from: &str, to: &str) {
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains(from), "{from} in {}", path.display());
    std::fs::write(&path, text.replace(from, to)).unwrap();
}

fn subagent(sc: &Scenario, file: &str) -> PathBuf {
    sc.projects()
        .join("-home-user-project")
        .join(S1)
        .join("subagents")
        .join(file)
}

#[test]
fn an_agent_without_descriptions_is_named_by_its_prompt_then_its_type() {
    let no_meta = |sc: &Scenario| {
        let meta = subagent(sc, "agent-a3explore000000001.meta.json");
        replace_in(
            meta,
            "\"description\": \"Explore invoice call sites\", ",
            "",
        );
    };
    let a = explore_after(no_meta);
    assert_eq!(
        (a.title.as_str(), a.title_source.as_str()),
        ("Explore invoice call sites", "spawner"),
        "the spawn's own description"
    );
    let no_spawn = |sc: &Scenario| {
        no_meta(sc);
        let parent = subagent(sc, "agent-a1author0000000001.jsonl");
        replace_in(
            parent,
            "\"description\":\"Explore invoice call sites\",",
            "",
        );
    };
    let a = explore_after(no_spawn);
    assert_eq!(
        (a.title.as_str(), a.title_source.as_str()),
        ("brief", "prompt")
    );
    let a = explore_after(|sc| {
        no_spawn(sc);
        let own = subagent(sc, "agent-a3explore000000001.jsonl");
        replace_in(own, "\"content\":\"brief\"", "\"content\":[]");
    });
    assert_eq!(
        (a.title.as_str(), a.title_source.as_str()),
        ("Explore a3explo", "type")
    );
}

#[test]
fn a_neutral_tab_label_leaves_the_root_to_its_session_and_a_pane_name_wins() {
    let sc = Scenario::new("demo");
    let mut snap = sc.snapshot().clone();
    for tab in snap["tabs"].as_array_mut().unwrap() {
        if tab["tab_id"] == "w1:t1" {
            tab["label"] = "claude".into(); // what herdr or Panorama itself would write
        }
    }
    let qs = sc.questions();
    let title_of = |snap: &Value| {
        let sw = sc
            .collector()
            .collect(Some(snap), Some(qs.as_slice()), sc.now(), 6.0 * H, STALE);
        let o = sw.roots.iter().find(|o| o.pane_id == "w1:p1").unwrap();
        (o.title.clone(), o.title_source.clone())
    };
    let prompt = "Take PR #42 and PR #44 through review";
    assert_eq!(title_of(&snap), (prompt.into(), "prompt".into()));
    let ai =
        format!(r#"{{"type":"ai-title","aiTitle":"Invoice export reviews","sessionId":"{S1}"}}"#);
    append(&s1_root(&sc), &format!("{ai}\n"));
    assert_eq!(
        title_of(&snap),
        ("Invoice export reviews".into(), "session".into())
    );
    snap["agents"][0]["name"] = "billing lead".into();
    assert_eq!(title_of(&snap), ("billing lead".into(), "pane".into()));
}
