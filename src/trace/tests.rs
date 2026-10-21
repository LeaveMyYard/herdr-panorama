use super::*;
use herdr_agent_kit::herdr_api::fake::{FakeHerdr, Reply};
use serde_json::{Value, json};

fn fixture() -> Value {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/trace/lines.json"
    );
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

#[test]
fn lines_are_pythons_byte_for_byte() {
    // captured from herdr.trace by tools/capture_trace.py: its rounding and its separators
    for case in fixture()["cases"].as_array().unwrap() {
        let got = line(
            case["op"].as_str().unwrap(),
            case["at_raw"].as_f64().unwrap(),
            case["ms_raw"].as_f64().unwrap(),
        );
        assert_eq!(got, case["line"].as_str().unwrap());
    }
}

#[test]
fn only_a_named_file_turns_it_on() {
    fn var(v: Option<&'static str>) -> impl Fn(&str) -> Option<std::ffi::OsString> {
        move |n: &str| {
            (n == VAR)
                .then(|| v.map(std::ffi::OsString::from))
                .flatten()
        }
    }
    assert_eq!(file(var(None)), None);
    assert_eq!(file(var(Some(""))), None);
    assert_eq!(
        file(var(Some("/t/x.jsonl"))),
        Some(PathBuf::from("/t/x.jsonl"))
    );
}

#[tokio::test(flavor = "current_thread")]
async fn each_herdr_call_is_one_line_named_as_python_names_it() {
    // tests/test_dock_panel.py::test_trace_records_each_herdr_call: a socket call and a CLI
    // call (failing: no herdr binary here) are both timed
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("trace.jsonl");
    let fake = FakeHerdr::start();
    fake.on("pane.layout", Reply::Result(json!({})));
    let h = fake
        .client()
        .with_bin(dir.path().join("no-herdr"))
        .with_observer(writer(path.clone()));
    let _ = h.raw("pane.layout", json!({"pane_id": "x"})).await;
    let _ = h.machine_snapshot("m1").await;
    let lines: Vec<Value> = std::fs::read_to_string(&path)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let ops: Vec<&str> = lines.iter().map(|l| l["op"].as_str().unwrap()).collect();
    assert_eq!(ops, ["socket pane.layout", "cli --machine m1"]);
    for l in &lines {
        assert!(
            l["at"].as_f64().unwrap() > 1.7e9 && l["ms"].as_f64().unwrap() >= 0.0,
            "{l}"
        );
    }
}

#[tokio::test(flavor = "current_thread")]
async fn at_is_when_the_call_ended() {
    // Python stamps the line when it writes it, after the call: a call that hangs to its
    // timeout ends at least that long after it started
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("trace.jsonl");
    let fake = FakeHerdr::start();
    fake.on("session.snapshot", Reply::Hang);
    let timeout = std::time::Duration::from_millis(300);
    let h = fake
        .client()
        .with_timeout(timeout)
        .with_observer(writer(path.clone()));
    let before = std::time::SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs_f64();
    let _ = h.raw("session.snapshot", json!({})).await;
    let l: Value = serde_json::from_str(std::fs::read_to_string(&path).unwrap().trim()).unwrap();
    assert!(l["at"].as_f64().unwrap() >= before + 0.29, "{l}");
    assert!(l["ms"].as_f64().unwrap() >= 299.0, "{l}");
}

#[test]
fn a_trace_file_that_cannot_be_written_never_fails_the_call() {
    let w = writer(PathBuf::from("/nonexistent-dir/x/trace.jsonl"));
    let fake = FakeHerdr::start();
    fake.on("pane.layout", Reply::Result(json!({})));
    let h = fake.blocking_client().with_observer(w);
    assert!(h.raw("pane.layout", json!({})).is_ok());
}

#[tokio::test(flavor = "current_thread")]
async fn panoramas_client_is_traced_exactly_when_the_variable_names_a_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("trace.jsonl");
    let fake = FakeHerdr::start();
    fake.on("pane.layout", Reply::Result(json!({})));
    let socket = fake.socket().as_os_str().to_owned();
    let env = |trace: Option<std::ffi::OsString>| {
        let socket = socket.clone();
        move |n: &str| match n {
            "HERDR_SOCKET_PATH" => Some(socket.clone()),
            VAR => trace.clone(),
            _ => None,
        }
    };
    let _ = client_from(env(None)).raw("pane.layout", json!({})).await;
    assert!(!path.exists());
    let traced = client_from(env(Some(path.clone().into_os_string())));
    let _ = traced.raw("pane.layout", json!({})).await;
    let b = traced.blocking();
    tokio::task::spawn_blocking(move || b.raw("pane.layout", json!({})))
        .await
        .unwrap()
        .ok();
    assert_eq!(std::fs::read_to_string(&path).unwrap().lines().count(), 2);
}
