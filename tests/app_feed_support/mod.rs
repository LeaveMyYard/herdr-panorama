#![allow(dead_code)]
use panorama::app_feed::{
    fs::{self, Home},
    receipt::{BINARY, FEED, FEED_LIMIT, Ownership, RECEIPT, Receipt, SMALL},
};
use serde_json::{Value, json};
use std::{
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::{Command, Output},
    time::{Duration, Instant},
};

pub const ID: &str = "0123456789abcdef0123456789abcdef";
pub struct Installed {
    pub dir: tempfile::TempDir,
    pub app: PathBuf,
    pub bin: PathBuf,
    pub data: PathBuf,
}
fn write(path: &Path, bytes: &[u8], mode: u32) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, bytes).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).unwrap();
}
impl Installed {
    pub fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let app = dir.path().join("app");
        let bin = dir.path().join("commands");
        let data = dir.path().join("fixtures");
        std::fs::create_dir_all(&app).unwrap();
        let app = app.canonicalize().unwrap();
        std::fs::create_dir(&bin).unwrap();
        std::fs::create_dir(&data).unwrap();
        let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("app");
        let mut files = serde_json::Map::new();
        for name in [
            "panorama",
            "panorama-spaces",
            "panorama-graph",
            "panorama-peek",
        ] {
            for file in ["main.js", "mod.json"] {
                let relative = format!("mods/{name}/{file}");
                let bytes = std::fs::read(source.join(&relative)).unwrap();
                write(&app.join(&relative), &bytes, 0o644);
                files.insert(relative, json!({"sha256":fs::digest(&bytes),"mode":420}));
            }
        }
        for relative in [
            "layouts/panorama.json",
            "mods/panorama/LICENSE.herdr-app",
            "mods/panorama/NOTICE",
        ] {
            let bytes = std::fs::read(source.join(relative)).unwrap();
            write(&app.join(relative), &bytes, 0o644);
            files.insert(
                relative.into(),
                json!({"sha256":fs::digest(&bytes),"mode":420}),
            );
        }
        std::fs::create_dir_all(app.join("data/panorama/bin")).unwrap();
        std::fs::copy(env!("CARGO_BIN_EXE_panorama"), app.join(BINARY)).unwrap();
        std::fs::set_permissions(app.join(BINARY), std::fs::Permissions::from_mode(0o755)).unwrap();
        let binary = std::fs::File::open(app.join(BINARY)).unwrap();
        files.insert(
            BINARY.into(),
            json!({"sha256":fs::file_digest(&binary).unwrap(),"mode":493}),
        );
        let gen_path = format!("data/panorama/runtime/{ID}");
        std::fs::create_dir_all(app.join(&gen_path)).unwrap();
        write(&app.join(&gen_path).join("writer.lock"), b"", 0o600);
        write(
            &app.join(&gen_path).join("control.json"),
            serde_json::to_string(&json!({"v":1,"install_id":ID,"disabled":false}))
                .unwrap()
                .as_bytes(),
            0o600,
        );
        write(&app.join(RECEIPT),serde_json::to_string(&json!({"v":2,"install_id":ID,"files":files,"dirs":["data","data/panorama","data/panorama/runtime",gen_path],"keys":null})).unwrap().as_bytes(),0o600);
        write(&data.join("state.json"), br#"{"status":"working"}"#, 0o600);
        write(&data.join("app-state"), b"running", 0o600);
        let root = serde_json::to_string(&data.to_string_lossy()).unwrap();
        let hc = format!(
            r#"#!/usr/bin/env python3
import json,sys,time,pathlib,os
root=pathlib.Path({root})
args=sys.argv[1:]
assert args in [['ls','--json'],['get','--json','t2'],['events','--json']]
with (root/'calls').open('a') as out: out.write(json.dumps(args)+'\n')
if args==['ls','--json']:
    state=json.loads((root/'state.json').read_text())
    if state.get('fail'): raise SystemExit(7)
    print(json.dumps({{'hidden_finished':0,'terminals':[{{'address':'t2','agent':'claude-code','cwd':'/fixture','state':'live','status':state['status'],'turn':1,'where':'fixture'}}]}}))
elif args[0]=='get': print(json.dumps({{'address':'t2','state':'live','foreground':None}}))
else:
    (root/'events.pid').write_text(str(os.getpid()))
    last=None
    while True:
        current=(root/'state.json').read_text()
        if current!=last: print(json.dumps({{'changed':True}}),flush=True);last=current
        time.sleep(0.02)
"#
        );
        write(&bin.join("hc"), hc.as_bytes(), 0o700);
        let app_status = format!(
            r#"#!/usr/bin/env python3
import sys,json,pathlib
assert sys.argv[1:]==['status']
root=pathlib.Path({root})
state=(root/'app-state').read_text()
if state=='running': print(json.dumps({{'layout':'fixture'}}))
elif state=='absent': print('herdr-app: no Herdr window at <isolated fixture>',file=sys.stderr);raise SystemExit(2)
else: raise SystemExit(7)
"#
        );
        write(&bin.join("herdr-app"), app_status.as_bytes(), 0o700);
        Self {
            dir,
            app,
            bin,
            data,
        }
    }
    /// Models an uninstall/reinstall generation transition without invoking B's installer.
    pub fn replace_generation(&self, next: &str) {
        assert!(
            !self
                .app
                .join(format!("data/panorama/runtime/{ID}/ownership.json"))
                .exists()
        );
        let receipt_path = self.app.join(RECEIPT);
        let mut receipt: Value =
            serde_json::from_slice(&std::fs::read(&receipt_path).unwrap()).unwrap();
        let old = format!("data/panorama/runtime/{ID}");
        let new = format!("data/panorama/runtime/{next}");
        std::fs::rename(self.app.join(&old), self.app.join(&new)).unwrap();
        receipt["install_id"] = json!(next);
        for dir in receipt["dirs"].as_array_mut().unwrap() {
            if dir.as_str() == Some(&old) {
                *dir = json!(new);
            }
        }
        std::fs::write(&receipt_path, serde_json::to_vec(&receipt).unwrap()).unwrap();
        let control = json!({"v":1,"install_id":next,"disabled":false});
        std::fs::write(
            self.app.join(new).join("control.json"),
            serde_json::to_vec(&control).unwrap(),
        )
        .unwrap();
    }
    pub fn command(&self) -> Command {
        let mut command = Command::new(self.app.join(BINARY));
        let inherited = std::env::var_os("PATH").unwrap_or_default();
        let path = std::env::join_paths(
            std::iter::once(self.bin.clone()).chain(std::env::split_paths(&inherited)),
        )
        .unwrap();
        command
            .env_clear()
            .env("PATH", path)
            .env("HOME", self.dir.path())
            .env("XDG_CONFIG_HOME", self.dir.path().join("config"))
            .env("XDG_STATE_HOME", self.dir.path().join("state"))
            .env("CLAUDE_CONFIG_DIR", self.dir.path().join("claude"))
            .env("CODEX_HOME", self.dir.path().join("codex"));
        command
    }
    pub fn ensure(&self) -> Output {
        self.command()
            .args(["app-feed", "--ensure", "--app-home"])
            .arg(&self.app)
            .output()
            .unwrap()
    }
    pub fn feed(&self) -> Value {
        serde_json::from_slice(&std::fs::read(self.app.join(FEED)).unwrap()).unwrap()
    }
    pub fn owner(&self) -> Ownership {
        let h = Home::open(&self.app).unwrap();
        let r = Receipt::read(&h).unwrap();
        Ownership::read(&h, &r).unwrap().unwrap().0
    }
    pub fn wait_feed(&self, check: impl Fn(&Value) -> bool) {
        self.wait_feed_within(Duration::from_secs(8), check)
    }
    pub fn wait_feed_within(&self, wait: Duration, check: impl Fn(&Value) -> bool) {
        let end = Instant::now() + wait;
        loop {
            if let Ok(bytes) = std::fs::read(self.app.join(FEED))
                && let Ok(v) = serde_json::from_slice(&bytes)
                && check(&v)
            {
                return;
            }
            assert!(Instant::now() < end, "feed condition not met");
            std::thread::sleep(Duration::from_millis(20));
        }
    }
    /// Emulates the agreed installer transaction, not a claim about B's installer implementation.
    pub fn quiesce(&self) -> std::result::Result<(Home, Receipt, std::fs::File), String> {
        self.quiesce_within(Duration::from_secs(2))
    }
    pub fn quiesce_within(
        &self,
        wait: Duration,
    ) -> std::result::Result<(Home, Receipt, std::fs::File), String> {
        let home = Home::open(&self.app)?;
        fs::lock(&home.fd, Duration::from_secs(2))?;
        let r = Receipt::read(&home)?;
        r.disable(&home)?;
        let lock = r.lock_file(&home)?;
        fs::lock(&lock, wait)?;
        r.check_names(&home)?;
        Ok((home, r, lock))
    }
    pub fn cleanup_runtime(home: &Home, r: &Receipt) {
        let owner = Ownership::read(home, r).unwrap();
        if let Some(feed) = home.read(FEED, FEED_LIMIT, 0o600).unwrap() {
            let hash = fs::digest(&feed.bytes);
            let o = &owner.as_ref().unwrap().0;
            assert!(
                o.feed_sha256.as_ref() == Some(&hash)
                    || o.pending_feed_sha256.as_ref() == Some(&hash)
            );
            home.remove(FEED, &feed).unwrap();
        }
        for file in ["ownership.json", "control.json", "writer.lock"] {
            if let Some(v) = home.read(&r.path(file), SMALL, 0o600).unwrap() {
                home.remove(&r.path(file), &v).unwrap();
            }
        }
        let receipt = home.read(RECEIPT, 1024 * 1024, 0o600).unwrap().unwrap();
        home.remove(RECEIPT, &receipt).unwrap();
    }
}
impl Drop for Installed {
    fn drop(&mut self) {
        if self.app.join(RECEIPT).exists()
            && let Ok((home, r, _lock)) = self.quiesce()
        {
            let _ = r.enabled(&home);
        }
    }
}
