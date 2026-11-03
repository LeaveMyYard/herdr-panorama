mod app_feed_support;
use app_feed_support::Installed;
use herdr_agent_kit::herdr_api::fake::{FakeHerdr, Reply};
use panorama::{
    app_feed::{
        fs::{self, Home},
        receipt::Receipt,
    },
    panel::act::inbox::INBOX_ID,
};
use serde_json::{Value, json};
use std::{
    os::unix::fs::PermissionsExt,
    path::Path,
    process::Command,
    time::{Duration, Instant},
};
fn executable(path: &Path, script: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, script).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}
fn plugin(install: &Installed, script: &str) -> FakeHerdr {
    let root = install.dir.path().join("inbox");
    executable(&root.join(".venv/bin/inbox"), script);
    let h = FakeHerdr::start();
    h.on("plugin.list",Reply::Result(json!({"plugins":[{"plugin_id":INBOX_ID,"plugin_root":root,"version":"0.10.5","enabled":true}]})));
    h
}
fn call(install: &Installed, h: &FakeHerdr, args: &[&str]) -> std::process::Output {
    install
        .command()
        .env("HERDR_SOCKET_PATH", h.socket())
        .args(["app-inbox"])
        .args(args)
        .arg("--app-home")
        .arg(&install.app)
        .output()
        .unwrap()
}
#[test]
fn help_invalid_arguments_and_unsafe_private_proof_do_no_tui_io() {
    let tmp = tempfile::tempdir().unwrap();
    let o = Command::new(env!("CARGO_BIN_EXE_panorama"))
        .env_clear()
        .env("HOME", tmp.path())
        .args(["app-inbox", "--help"])
        .output()
        .unwrap();
    assert!(o.status.success());
    assert!(String::from_utf8_lossy(&o.stdout).starts_with("panorama app-inbox v1\n"));
    assert_eq!(std::fs::read_dir(tmp.path()).unwrap().count(), 0);
    let i = Installed::new();
    let h = FakeHerdr::start();
    assert_eq!(
        call(&i, &h, &["--open", "--question", "-1"]).status.code(),
        Some(2)
    );
    let home = Home::open(&i.app).unwrap();
    let r = Receipt::read(&home).unwrap();
    r.disable(&home).unwrap();
    let o = call(&i, &h, &["--available"]);
    assert_eq!(o.status.code(), Some(1));
    assert!(o.stdout.is_empty());
    assert!(h.calls().is_empty());
}
#[test]
fn capability_and_open_use_only_existing_installed_navigation() {
    let i = Installed::new();
    let record = i.dir.path().join("args");
    let script = format!(
        "#!/usr/bin/env python3\nimport sys,json,os,pathlib\nassert os.environ['PYTHONDONTWRITEBYTECODE']=='1'\nassert not any(k.startswith('HC_') for k in os.environ)\nif sys.argv[1:]==['open','--help']: print('--question --no-hold')\nelse: pathlib.Path({:?}).write_text(json.dumps(sys.argv[1:]))\n",
        record.to_str().unwrap()
    );
    let h = plugin(&i, &script);
    let o = call(&i, &h, &["--available"]);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    assert_eq!(
        serde_json::from_slice::<Value>(&o.stdout).unwrap(),
        json!({"available":true,"questions":true})
    );
    assert!(!record.exists());
    let o = call(&i, &h, &["--open", "--question", "14"]);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    assert_eq!(
        serde_json::from_slice::<Value>(&o.stdout).unwrap(),
        json!({"opened":true})
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&std::fs::read(&record).unwrap()).unwrap(),
        json!(["open", "--question", "Q14", "--no-hold"])
    );
    assert!(h.calls().iter().all(|(method, _)| method == "plugin.list"));
}
#[test]
fn disabled_after_capability_query_is_revalidated_before_open() {
    let i = Installed::new();
    let path = i.app.join(format!(
        "data/panorama/runtime/{}/control.json",
        app_feed_support::ID
    ));
    let record = i.dir.path().join("opened");
    let script = format!(
        "#!/usr/bin/env python3\nimport sys,json,pathlib\nif sys.argv[1:]==['open','--help']:\n p=pathlib.Path({:?});d=json.loads(p.read_text());d['disabled']=True;p.write_text(json.dumps(d));print('--question')\nelse: pathlib.Path({:?}).write_text('unexpected')\n",
        path.to_str().unwrap(),
        record.to_str().unwrap()
    );
    let h = plugin(&i, &script);
    let o = call(&i, &h, &["--open", "--question", "14"]);
    assert_eq!(o.status.code(), Some(1));
    assert!(o.stdout.is_empty());
    assert!(!record.exists());
}
#[test]
fn unavailable_is_explicit_and_whole_operation_deadline_reaps_owned_helper() {
    let i = Installed::new();
    let h = FakeHerdr::start();
    h.on("plugin.list", Reply::Result(json!({"plugins":[]})));
    let o = call(&i, &h, &["--available"]);
    assert!(o.status.success());
    assert_eq!(
        serde_json::from_slice::<Value>(&o.stdout).unwrap(),
        json!({"available":false,"questions":false})
    );
    let marker = i.dir.path().join("helper.pid");
    let script = format!(
        "#!/usr/bin/env python3\nimport pathlib,os,time\npathlib.Path({:?}).write_text(str(os.getpid()))\ntime.sleep(20)\n",
        marker.to_str().unwrap()
    );
    let h = plugin(&i, &script);
    for action in ["--available", "--open", "--available"] {
        let began = Instant::now();
        let o = call(&i, &h, &[action]);
        assert!(
            began.elapsed() < Duration::from_secs(5),
            "{:?}",
            began.elapsed()
        );
        assert_eq!(
            o.status.code(),
            Some(1),
            "{action}: {}",
            String::from_utf8_lossy(&o.stdout)
        );
        assert!(o.stdout.is_empty());
        let pid = std::fs::read_to_string(&marker)
            .unwrap()
            .parse::<u32>()
            .unwrap();
        assert!(herdr_agent_kit::agent_procs::start_token(pid).is_none());
    }
    assert_eq!(h.calls().len(), 3);
    // A contended lifecycle lock consumes the same total budget; no plugin call follows.
    let home = Home::open(&i.app).unwrap();
    fs::lock(&home.fd, Duration::from_millis(1)).unwrap();
    let before = h.calls().len();
    let began = Instant::now();
    let o = call(&i, &h, &["--available"]);
    assert_eq!(o.status.code(), Some(1));
    assert!(began.elapsed() < Duration::from_secs(5));
    assert_eq!(h.calls().len(), before);
}

#[test]
fn old_generation_after_reinstall_never_queries_or_opens_inbox() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/app-feed-runtime/lifecycle.json")).unwrap();
    let next = fixture["generation_fence"]["replacement_install_id"]
        .as_str()
        .unwrap();
    let i = Installed::new();
    i.replace_generation(next);
    let h = FakeHerdr::start();
    h.on("plugin.list", Reply::Result(json!({"plugins":[]})));
    for action in ["--available", "--open"] {
        let o = call(&i, &h, &[action, "--install-id", app_feed_support::ID]);
        assert_eq!(o.status.code(), Some(1));
        assert!(o.stdout.is_empty());
        assert!(h.calls().is_empty());
    }
    let o = call(&i, &h, &["--available", "--install-id", next]);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    assert_eq!(
        serde_json::from_slice::<Value>(&o.stdout).unwrap(),
        json!({"available":false,"questions":false})
    );
    assert_eq!(h.calls().len(), 1);
    for args in [
        vec!["--available", "--install-id", "bad"],
        vec!["--open", "--install-id", next, "--install-id", next],
    ] {
        assert_eq!(call(&i, &h, &args).status.code(), Some(2));
        assert_eq!(h.calls().len(), 1);
    }
}

#[test]
fn plugin_lookup_transport_timeout_is_an_operation_error_before_any_helper() {
    let i = Installed::new();
    let marker = i.dir.path().join("unexpected-inbox-helper");
    let script = format!(
        "#!/usr/bin/env python3\nimport pathlib\npathlib.Path({:?}).write_text('unexpected')\nprint('--question')\n",
        marker.to_str().unwrap()
    );
    let h = plugin(&i, &script);
    h.on("plugin.list", Reply::Hang);
    let began = Instant::now();
    let out = call(&i, &h, &["--available"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(out.stdout.is_empty());
    let diagnostic = String::from_utf8_lossy(&out.stderr);
    assert!(
        diagnostic.contains("plugin lookup failed") && diagnostic.contains("timed out"),
        "{diagnostic}"
    );
    assert!(
        began.elapsed() < Duration::from_secs(5),
        "{:?}",
        began.elapsed()
    );
    assert_eq!(h.calls().len(), 1);
    assert_eq!(h.calls()[0].0, "plugin.list");
    assert!(!marker.exists());
}
