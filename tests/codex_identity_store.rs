//! Public resolver regressions for the association store's filesystem trust boundary.
use herdr_agent_kit::{
    agent_procs::{ProcessTable, SystemProcesses},
    plugin_env::PluginEnv,
};
use panorama::codex::identity::resolve;
use serde_json::{Value, json};
use std::{
    os::unix::{
        fs::{MetadataExt, PermissionsExt, symlink},
        process::CommandExt,
    },
    process::{Child, Command},
    time::{Duration, Instant},
};
const ROOT: &str = "11111111-1111-4111-8111-111111111111";
struct Probe {
    temp: tempfile::TempDir,
    child: Child,
    env: PluginEnv,
}
impl Probe {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let uid = std::fs::metadata(temp.path()).unwrap().uid();
        let env = PluginEnv::from_vars(
            [("XDG_STATE_HOME", temp.path().to_str().unwrap())],
            None,
            uid,
        );
        let child = Command::new("/bin/sleep")
            .arg0("codex")
            .arg("30")
            .spawn()
            .unwrap();
        let mut probe = Self { temp, child, env };
        let owner_start = probe.ready();
        let parent = probe.temp.path().join("herdr-panorama");
        let dir = parent.join("codex-bindings");
        std::fs::create_dir_all(&dir).unwrap();
        for path in [&parent, &dir] {
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700)).unwrap();
        }
        let file = dir.join("77313a7031");
        std::fs::write(&file,json!({"version":1,"pane_id":"w1:p1","thread_id":ROOT,"owner_pid":probe.child.id(),"owner_start":owner_start,"embedded":false,"conflicted":false}).to_string()).unwrap();
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600)).unwrap();
        probe
    }
    fn ready(&mut self) -> String {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let pid = self.child.id();
            let status = self.child.try_wait().unwrap();
            assert!(
                status.is_none(),
                "owned fixture exited before readiness: {status:?}"
            );
            // spawn success can precede observable argv on procfs. Prove the
            // intended owned process before publishing its association.
            let table = SystemProcesses::new();
            let start = table.start(pid);
            let argv = table.cmdline(pid);
            if argv == ["codex", "30"]
                && start.as_ref().is_some_and(|value| !value.is_empty())
                && SystemProcesses::new().start(pid) == start
            {
                return start.unwrap();
            }
            assert!(
                Instant::now() < deadline,
                "owned fixture argv not ready: pid={pid}, start={start:?}, argv={argv:?}"
            );
            std::thread::yield_now();
        }
    }
    fn info(&self) -> Value {
        json!({"process_info":{"pane_id":"w1:p1","foreground_processes":[{"pid":self.child.id()}]}})
    }
    fn rejected(&self) {
        assert_eq!(
            resolve(&self.env, "w1:p1", None, || panic!(
                "untrusted state must not request process information"
            )),
            None
        );
        assert_eq!(
            resolve(&self.env, "w1:p1", Some(ROOT), || panic!("native wins")),
            Some(ROOT.into())
        );
    }
}
impl Drop for Probe {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[test]
fn public_resolver_rejects_writable_and_symlinked_binding_directory_or_parent() {
    for relative in ["herdr-panorama/codex-bindings", "herdr-panorama"] {
        let p = Probe::new();
        assert_eq!(
            resolve(&p.env, "w1:p1", None, || Some(p.info())),
            Some(ROOT.into())
        );
        let path = p.temp.path().join(relative);
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o777)).unwrap();
        p.rejected();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
        let moved = p.temp.path().join("moved");
        std::fs::rename(&path, &moved).unwrap();
        symlink(&moved, &path).unwrap();
        p.rejected();
    }
}
#[test]
fn public_resolver_rejects_untrusted_file_integrity_and_wrong_directory_owner() {
    for mode in [0o666, 0o644] {
        let p = Probe::new();
        let path = p
            .temp
            .path()
            .join("herdr-panorama/codex-bindings/77313a7031");
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).unwrap();
        p.rejected();
    }
    for symlinked in [false, true] {
        let p = Probe::new();
        let path = p
            .temp
            .path()
            .join("herdr-panorama/codex-bindings/77313a7031");
        let moved = p.temp.path().join("binding");
        if symlinked {
            std::fs::rename(&path, &moved).unwrap();
            symlink(&moved, &path).unwrap();
        } else {
            std::fs::hard_link(&path, &moved).unwrap();
        }
        p.rejected();
    }
    let p = Probe::new();
    let uid = std::fs::metadata(p.temp.path()).unwrap().uid();
    let wrong = PluginEnv::from_vars(
        [("XDG_STATE_HOME", p.temp.path().to_str().unwrap())],
        None,
        uid + 1,
    );
    assert_eq!(
        resolve(&wrong, "w1:p1", None, || panic!("wrong owner")),
        None
    );
}

#[test]
fn a_trusted_base_can_have_a_system_style_symlink_ancestor() {
    // macOS commonly spells /private/var/... as /var/... . Open the supplied
    // state base once, then anchor the plugin parent/private directory via openat.
    let p = Probe::new();
    let aliases = tempfile::tempdir().unwrap();
    let ancestor = aliases.path().join("system-path");
    symlink(p.temp.path().parent().unwrap(), &ancestor).unwrap();
    let base = ancestor.join(p.temp.path().file_name().unwrap());
    let uid = std::fs::metadata(&base).unwrap().uid();
    let env = PluginEnv::from_vars([("XDG_STATE_HOME", base.to_str().unwrap())], None, uid);
    assert_eq!(
        resolve(&env, "w1:p1", None, || Some(p.info())),
        Some(ROOT.into())
    );
}
