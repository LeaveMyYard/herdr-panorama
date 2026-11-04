mod app_feed_support;
use app_feed_support::Installed;
use panorama::app_feed::{
    fs::{self, Home},
    receipt::{BINARY, Ownership, RECEIPT, Receipt, SMALL},
};
use serde_json::json;
use std::{os::unix::fs::PermissionsExt, process::Command, time::Duration};

#[test]
fn help_and_invalid_installations_never_start_or_create_runtime() {
    let temp = tempfile::tempdir().unwrap();
    let help = Command::new(env!("CARGO_BIN_EXE_panorama"))
        .env_clear()
        .args(["app-feed", "--help"])
        .env("HOME", temp.path())
        .env("HERDR_APP_HOME", temp.path().join("missing"))
        .output()
        .unwrap();
    assert!(help.status.success());
    assert!(String::from_utf8_lossy(&help.stdout).starts_with("panorama app-feed v1\n"));
    assert_eq!(std::fs::read_dir(temp.path()).unwrap().count(), 0);
    let install = Installed::new();
    let before = std::fs::read(install.app.join(RECEIPT)).unwrap();
    std::fs::set_permissions(
        install.app.join(BINARY),
        std::fs::Permissions::from_mode(0o700),
    )
    .unwrap();
    let out = install.ensure();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("hash/mode mismatch"));
    assert!(
        !install
            .app
            .join(format!(
                "data/panorama/runtime/{}/ownership.json",
                app_feed_support::ID
            ))
            .exists()
    );
    assert_eq!(std::fs::read(install.app.join(RECEIPT)).unwrap(), before);
    assert!(!install.data.join("calls").exists());

    std::fs::set_permissions(
        install.app.join(BINARY),
        std::fs::Permissions::from_mode(0o755),
    )
    .unwrap();
    let lock_path = install.app.join(format!(
        "data/panorama/runtime/{}/writer.lock",
        app_feed_support::ID
    ));
    std::fs::remove_file(&lock_path).unwrap();
    let outside = install.dir.path().join("personal-lock");
    std::fs::write(&outside, "personal").unwrap();
    std::os::unix::fs::symlink(&outside, &lock_path).unwrap();
    assert!(!install.ensure().status.success());
    assert_eq!(std::fs::read_to_string(&outside).unwrap(), "personal");
    std::fs::remove_file(lock_path).unwrap();
    let mut legacy: serde_json::Value = serde_json::from_slice(&before).unwrap();
    legacy["v"] = json!(1);
    legacy.as_object_mut().unwrap().remove("install_id");
    std::fs::write(
        install.app.join(RECEIPT),
        serde_json::to_vec(&legacy).unwrap(),
    )
    .unwrap();
    assert!(!install.ensure().status.success());
    assert!(!install.data.join("calls").exists());
}

#[test]
fn concurrent_ensure_is_one_writer_and_events_recollect_degraded_errors() {
    let install = Installed::new();
    let mut children = Vec::new();
    for _ in 0..2 {
        children.push(
            install
                .command()
                .args(["app-feed", "--ensure", "--app-home"])
                .arg(&install.app)
                .spawn()
                .unwrap(),
        );
    }
    for mut child in children {
        assert!(child.wait().unwrap().success());
    }
    install.wait_feed(|v| v["orchestrators"][0]["herdr_status"] == "working");
    let owner = install.owner();
    let initial = install.ensure();
    assert!(
        initial.status.success(),
        "{}",
        String::from_utf8_lossy(&initial.stderr)
    );
    assert_eq!(install.owner().writer_pid, owner.writer_pid);
    assert_eq!(install.owner().writer_start, owner.writer_start);
    std::fs::write(install.data.join("state.json"), br#"{"status":"blocked"}"#).unwrap();
    install.wait_feed(|v| v["orchestrators"][0]["herdr_status"] == "blocked");
    std::fs::write(
        install.data.join("state.json"),
        br#"{"status":"working","fail":true}"#,
    )
    .unwrap();
    install.wait_feed(|v| v["herdr_ok"] == false && !v["errors"].as_array().unwrap().is_empty());
    let (_home, _r, _lock) = install.quiesce().unwrap();
}

#[test]
fn used_home_stop_and_receipt_removal_block_a_delayed_restart() {
    let install = Installed::new();
    let initial = install.ensure();
    assert!(
        initial.status.success(),
        "{}",
        String::from_utf8_lossy(&initial.stderr)
    );
    install.wait_feed(|v| v["v"] == 1);
    let personal = install.app.join("personal.txt");
    std::fs::write(&personal, "personal").unwrap();
    let (home, r, _lock) = install.quiesce().unwrap();
    let mut delayed = install
        .command()
        .args(["app-feed", "--ensure", "--app-home"])
        .arg(&install.app)
        .spawn()
        .unwrap();
    std::thread::sleep(Duration::from_millis(50));
    assert!(delayed.try_wait().unwrap().is_none());
    Installed::cleanup_runtime(&home, &r);
    drop(home);
    assert!(!delayed.wait().unwrap().success());
    assert!(!install.app.join(RECEIPT).exists());
    assert_eq!(std::fs::read_to_string(personal).unwrap(), "personal");
    assert!(!install.ensure().status.success());
}

#[test]
fn unknown_runtime_files_preserve_receipt_and_personal_data_after_stop() {
    let install = Installed::new();
    let initial = install.ensure();
    assert!(
        initial.status.success(),
        "{}",
        String::from_utf8_lossy(&initial.stderr)
    );
    install.wait_feed(|v| v["v"] == 1);
    let path = install.app.join(format!(
        "data/panorama/runtime/{}/personal.txt",
        app_feed_support::ID
    ));
    std::fs::write(&path, "personal").unwrap();
    assert!(install.quiesce().is_err());
    assert!(install.app.join(RECEIPT).exists());
    assert_eq!(std::fs::read_to_string(path).unwrap(), "personal");
    let home = Home::open(&install.app).unwrap();
    let r = Receipt::read(&home).unwrap();
    assert!(r.control(&home).unwrap().0.disabled);
    assert!(!install.ensure().status.success());
}

#[test]
fn a_busy_lock_with_reused_or_wrong_command_pid_is_not_adopted() {
    let install = Installed::new();
    let home = Home::open(&install.app).unwrap();
    let r = Receipt::read(&home).unwrap();
    let lock = r.lock_file(&home).unwrap();
    fs::lock(&lock, Duration::ZERO).unwrap();
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/app-feed-runtime/lifecycle.json")).unwrap();
    let mut owner = fixture["initial_ownership"].clone();
    owner["binary_sha256"] = json!(r.binary_sha);
    owner["writer_pid"] = json!(std::process::id());
    owner["writer_start"] = json!("reused-start");
    let bytes = serde_json::to_vec(&owner).unwrap();
    home.replace(
        &r.path("ownership.json"),
        &r.path(".ownership.tmp"),
        &bytes,
        None,
    )
    .unwrap();
    for start in [
        "reused-start".to_owned(),
        herdr_agent_kit::agent_procs::start_token(std::process::id()).unwrap(),
    ] {
        let old = home
            .read(&r.path("ownership.json"), SMALL, 0o600)
            .unwrap()
            .unwrap();
        owner["writer_start"] = json!(start);
        home.replace(
            &r.path("ownership.json"),
            &r.path(".ownership.tmp"),
            &serde_json::to_vec(&owner).unwrap(),
            Some(&old),
        )
        .unwrap();
        let out = install.ensure();
        assert!(!out.status.success());
        assert!(String::from_utf8_lossy(&out.stderr).contains("ownership cannot be proven"));
    }
    let receipt_before = std::fs::read(install.app.join(RECEIPT)).unwrap();
    let started = std::time::Instant::now();
    assert!(install.quiesce().is_err());
    assert!(
        started.elapsed() >= Duration::from_secs(2) && started.elapsed() < Duration::from_secs(3)
    );
    assert_eq!(
        std::fs::read(install.app.join(RECEIPT)).unwrap(),
        receipt_before
    );
    assert!(r.control(&home).unwrap().0.disabled);
    drop(lock);
    let _ = Ownership::read(&home, &r).unwrap();
}

#[test]
fn stale_registration_with_a_free_lifetime_lock_can_start_a_new_proven_writer() {
    let install = Installed::new();
    let home = Home::open(&install.app).unwrap();
    let r = Receipt::read(&home).unwrap();
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/app-feed-runtime/lifecycle.json")).unwrap();
    let mut owner = fixture["initial_ownership"].clone();
    owner["binary_sha256"] = json!(r.binary_sha);
    owner["writer_pid"] = json!(4_000_000_000_u32);
    owner["writer_start"] = json!("obsolete-start");
    home.replace(
        &r.path("ownership.json"),
        &r.path(".ownership.tmp"),
        &serde_json::to_vec(&owner).unwrap(),
        None,
    )
    .unwrap();
    let result = install.ensure();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    install.wait_feed(|v| v["v"] == 1);
    let current = install.owner();
    assert_ne!(current.writer_pid, 4_000_000_000_u32);
    assert_ne!(current.writer_start, "obsolete-start");
    assert!(current.live(&home));
    let (_home, _r, _lock) = install.quiesce().unwrap();
}

#[test]
fn stop_while_a_collection_helper_is_blocked_reaps_it_before_unlocking() {
    let install = Installed::new();
    let result = install.ensure();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    install.wait_feed(|v| v["v"] == 1);
    let script = install.bin.join("hc");
    let blocked = install.data.join("blocked.pid");
    let text = format!(
        "#!/usr/bin/env python3\nimport os,pathlib,time\npathlib.Path({:?}).write_text(str(os.getpid()))\ntime.sleep(30)\n",
        blocked.to_string_lossy()
    );
    std::fs::write(&script, text).unwrap();
    let deadline = std::time::Instant::now() + Duration::from_secs(8);
    while !blocked.exists() {
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(20));
    }
    let pid = std::fs::read_to_string(&blocked)
        .unwrap()
        .parse::<u32>()
        .unwrap();
    let before = herdr_agent_kit::agent_procs::start_token(pid).unwrap();
    let started = std::time::Instant::now();
    let (_home, _r, _lock) = install.quiesce().unwrap();
    assert!(started.elapsed() < Duration::from_secs(2));
    assert_ne!(
        herdr_agent_kit::agent_procs::start_token(pid).as_ref(),
        Some(&before)
    );
}

#[test]
fn cooperative_runtime_stop_never_executes_an_edited_private_binary() {
    let install = Installed::new();
    let result = install.ensure();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    install.wait_feed(|v| v["v"] == 1);
    let marker = install.data.join("edited-binary-executed");
    let replacement = install.app.join("data/panorama/bin/replacement");
    let bytes = format!(
        "#!/usr/bin/env python3\nimport pathlib\npathlib.Path({:?}).touch()\n",
        marker.to_string_lossy()
    )
    .into_bytes();
    std::fs::write(&replacement, &bytes).unwrap();
    std::fs::set_permissions(&replacement, std::fs::Permissions::from_mode(0o755)).unwrap();
    std::fs::rename(replacement, install.app.join(BINARY)).unwrap();
    let (home, r, _lock) = install.quiesce().unwrap();
    let owner = Ownership::read(&home, &r).unwrap().unwrap().0;
    assert_eq!(owner.binary_sha256, r.binary_sha);
    Installed::cleanup_runtime(&home, &r);
    assert_eq!(std::fs::read(install.app.join(BINARY)).unwrap(), bytes);
    assert!(!marker.exists());
    assert!(!install.app.join(RECEIPT).exists());
}

#[test]
fn old_generation_cannot_start_a_reinstalled_writer_but_current_generation_can() {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/app-feed-runtime/lifecycle.json")).unwrap();
    let next = fixture["generation_fence"]["replacement_install_id"]
        .as_str()
        .unwrap();
    let installed = Installed::new();
    installed.replace_generation(next);
    let attempt = |id: &str| {
        installed
            .command()
            .args(["app-feed", "--ensure", "--install-id", id, "--app-home"])
            .arg(&installed.app)
            .output()
            .unwrap()
    };
    let out = attempt(app_feed_support::ID);
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("generation changed"));
    assert!(
        !installed
            .app
            .join(format!("data/panorama/runtime/{next}/ownership.json"))
            .exists()
    );
    assert!(!installed.data.join("calls").exists());
    let out = attempt(next);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    installed.wait_feed(|v| v["orchestrators"][0]["terminal"] == "t2");
    assert_eq!(installed.owner().install_id, next);
}

#[test]
fn malformed_or_duplicate_generation_arguments_fail_before_home_io() {
    let temp = tempfile::tempdir().unwrap();
    for ids in [
        vec!["--install-id", "bad"],
        vec![
            "--install-id",
            app_feed_support::ID,
            "--install-id",
            app_feed_support::ID,
        ],
    ] {
        let out = Command::new(env!("CARGO_BIN_EXE_panorama"))
            .env_clear()
            .args(["app-feed", "--ensure", "--app-home"])
            .arg(temp.path().join("absent"))
            .args(ids)
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(2));
        assert!(out.stdout.is_empty());
        assert_eq!(std::fs::read_dir(temp.path()).unwrap().count(), 0);
    }
}

#[test]
fn a_foreign_generation_staging_file_refuses_start_without_changing_bytes() {
    let installed = Installed::new();
    let foreign = installed
        .app
        .join("data/panorama/.feed-fedcba9876543210fedcba9876543210.tmp");
    let bytes = b"{\"v\":1,\"personal\":true}\n";
    std::fs::write(&foreign, bytes).unwrap();
    let receipt = std::fs::read(installed.app.join(RECEIPT)).unwrap();
    let out = installed.ensure();
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("unknown runtime parent entry"));
    assert_eq!(std::fs::read(&foreign).unwrap(), bytes);
    assert_eq!(std::fs::read(installed.app.join(RECEIPT)).unwrap(), receipt);
    assert!(
        !installed
            .app
            .join(format!(
                "data/panorama/runtime/{}/ownership.json",
                app_feed_support::ID
            ))
            .exists()
    );
    assert!(!installed.data.join("calls").exists());
}

fn ensure_without_processes(install: &Installed) -> std::process::Output {
    // A zero process limit makes the validated writer spawn fail like an overloaded host.
    let command = install.command();
    let mut shell = Command::new("bash");
    shell
        .env_clear()
        .envs(command.get_envs().filter_map(|(k, v)| Some((k, v?))));
    shell
        .args(["-c", "ulimit -u 0 && exec \"$0\" \"$@\""])
        .arg(command.get_program())
        .args(["app-feed", "--ensure", "--app-home"])
        .arg(&install.app);
    shell.output().unwrap()
}

fn ledger_path(install: &Installed) -> std::path::PathBuf {
    install.app.join(format!(
        "data/panorama/runtime/{}/start.json",
        app_feed_support::ID
    ))
}

fn write_ledger(install: &Installed, failures: u64, retry_at_ms: u64) {
    let path = ledger_path(install);
    let value = json!({"v":1,"install_id":app_feed_support::ID,"failures":failures,"retry_at_ms":retry_at_ms});
    std::fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
}

fn disabled(install: &Installed) -> bool {
    let home = Home::open(&install.app).unwrap();
    let r = Receipt::read(&home).unwrap();
    r.control(&home).unwrap().0.disabled
}

#[test]
fn a_transient_spawn_failure_backs_off_then_retries_without_disabling() {
    let install = Installed::new();
    let failed = ensure_without_processes(&install);
    let stderr = String::from_utf8_lossy(&failed.stderr);
    assert_eq!(failed.status.code(), Some(1), "{stderr}");
    assert!(stderr.contains("transient start failure 1/8"), "{stderr}");
    assert!(!disabled(&install));
    let ledger: serde_json::Value =
        serde_json::from_slice(&std::fs::read(ledger_path(&install)).unwrap()).unwrap();
    assert_eq!(ledger["failures"], 1);
    // Within the backoff a later ensure defers without spawning anything.
    let deferred = install.ensure();
    assert!(String::from_utf8_lossy(&deferred.stderr).contains("writer start deferred"));
    assert!(!install.data.join("calls").exists());
    assert!(!disabled(&install));
    write_ledger(&install, 1, 0);
    let retried = install.ensure();
    assert!(
        retried.status.success(),
        "{}",
        String::from_utf8_lossy(&retried.stderr)
    );
    assert!(!ledger_path(&install).exists());
    install.wait_feed(|v| v["v"].is_u64());
    let (_home, _r, _lock) = install.quiesce().unwrap();
}

#[test]
fn the_bounded_transient_failure_count_disables_the_generation() {
    let install = Installed::new();
    write_ledger(&install, 7, 0);
    let failed = ensure_without_processes(&install);
    let stderr = String::from_utf8_lossy(&failed.stderr);
    assert!(
        stderr.contains("8 consecutive transient start failures"),
        "{stderr}"
    );
    assert!(disabled(&install));
    assert!(!install.ensure().status.success());
}

#[test]
fn an_ambiguous_start_ledger_refuses_start_without_changing_control() {
    for (name, bytes) in [
        ("start.json", br#"{"v":1,"install_id":"0123456789abcdef0123456789abcdef","failures":0,"retry_at_ms":0}"#.as_slice()),
        ("start.json", br#"{"v":1,"install_id":"0123456789abcdef0123456789abcdef","failures":1,"retry_at_ms":0,"x":1}"#.as_slice()),
        (".start.tmp", b"{partial".as_slice()),
    ] {
        let install = Installed::new();
        let path = install.app.join(format!(
            "data/panorama/runtime/{}/{name}",
            app_feed_support::ID
        ));
        std::fs::write(&path, bytes).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        let out = install.ensure();
        assert_eq!(out.status.code(), Some(1));
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        assert!(!disabled(&install));
        assert!(!install.data.join("calls").exists());
    }
}

#[test]
fn a_registered_writer_acknowledging_late_is_kept_not_disabled() {
    let install = Installed::new();
    let out = install
        .command()
        .env("PANORAMA_TEST_READY_DELAY_MS", "6500")
        .args(["app-feed", "--ensure", "--app-home"])
        .arg(&install.app)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(!disabled(&install));
    let owner = install.owner();
    install.wait_feed(|v| v["v"].is_u64());
    assert_eq!(install.owner().writer_pid, owner.writer_pid);
    let (_home, _r, _lock) = install.quiesce().unwrap();
}
