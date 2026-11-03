use super::*;
use herdr_agent_kit::agent_procs::fake::FakeProcesses;
use std::os::unix::fs::PermissionsExt;

pub(crate) fn fake_core(body: &str) -> (tempfile::TempDir, Core) {
    let dir = tempfile::tempdir().unwrap();
    let binary = dir.path().join("hc");
    std::fs::write(&binary, format!("#!/usr/bin/env python3\n{body}\n")).unwrap();
    std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o700)).unwrap();
    let core = Core::new(binary.into_os_string(), Duration::from_millis(300));
    (dir, core)
}

#[test]
fn core_command_is_bounded_read_only_and_passes_address_as_one_argument() {
    let (_dir, core) = fake_core(
        "import json,sys,os\nassert 'HC_TERMINAL_ID' not in os.environ\nassert sys.argv[1:] == ['get','--json','vps/t7']\nprint(json.dumps({'address':'vps/t7'}))",
    );
    assert_eq!(core.terminal("vps/t7").unwrap()["address"], "vps/t7");
    assert!(core.terminal("t7 --kill").is_err());
    for body in [
        "print('bad JSON')",
        "raise SystemExit(7)",
        "import time\ntime.sleep(30)",
    ] {
        let (_dir, core) = fake_core(body);
        let start = Instant::now();
        assert!(core.list().is_err());
        assert!(start.elapsed() < Duration::from_secs(2));
    }
}

#[test]
fn exact_current_foreground_and_start_token_are_required() {
    let terminal = json!({"address":"t2","state":"live","foreground":{"pid":20,"name":"codex","argv":["codex","--no-daemon"],"cwd":"/fixture"}});
    let info = process(&terminal, "t2").unwrap();
    let mut table = FakeProcesses::default();
    table.insert(20, 1, "codex", "boot-1", &["codex", "--no-daemon"]);
    assert!(owns(&info, "core:t2", 20, "boot-1", &table));
    assert!(!owns(&info, "core:t3", 20, "boot-1", &table));
    assert!(!owns(&info, "core:t2", 20, "old-start", &table));
    assert!(process(&terminal, "t3").is_none());
    let mut terminal = terminal;
    terminal["state"] = json!("finished");
    assert!(process(&terminal, "t2").is_none());
}
