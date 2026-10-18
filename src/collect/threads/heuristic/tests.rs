use super::*;
use panorama_core::model::{Agent, Status};

fn agent(id: &str, desc: &str, role: &str, started: f64, status: Status) -> Agent {
    Agent {
        id: id.into(),
        description: desc.into(),
        role: role.into(),
        started: Some(started),
        status,
        ..Agent::default()
    }
}

fn arena(agents: Vec<Agent>) -> Arena {
    agents.into_iter().map(|a| (a.id.clone(), a)).collect()
}

#[test]
fn a_bare_reference_joins_the_one_repo_thread_and_turns_take_the_rest() {
    let mut ar = arena(vec![
        agent("a", "author herdr #42", "author", 1.0, Status::Done),
        agent("b", "review #42", "reviewer", 2.0, Status::Running),
        agent("c", "look around", "explorer", 3.0, Status::Running),
        agent("d", "and more", "explorer", 4.0, Status::Running),
    ]);
    ar["a"].ended = Some(1.5);
    let ids: Vec<String> = ["a", "b", "c", "d"].map(String::from).to_vec();
    let turn_of = HashMap::from([("c".to_owned(), 2), ("d".to_owned(), 2)]);
    let out = group_refs_and_turns(&ids, &turn_of, &HashMap::new(), &mut ar);
    let names: Vec<_> = out
        .iter()
        .map(|d| (d.thread.id.as_str(), d.thread.name.as_str()))
        .collect();
    assert_eq!(
        names,
        [("ref:herdr#42", "herdr #42"), ("turn:2", "look around +1")]
    );
    assert_eq!(out[0].ids, ["a", "b"]);
    let phases: Vec<_> = out[0]
        .thread
        .phases
        .iter()
        .map(|p| (p.name.as_str(), p.state.code()))
        .collect();
    assert_eq!(phases, [("author", "done"), ("review r1", "now")]);
    assert_eq!(ar["b"].phases, [1]);
}
