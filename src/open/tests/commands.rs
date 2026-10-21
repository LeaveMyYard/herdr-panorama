use super::super::*;
use std::sync::Mutex;

/// What the commands asked herdr and the core.
#[derive(Default)]
struct Fake {
    core: Option<&'static str>,
    ensure_fails: Option<&'static str>,
    view_fails: bool,
    calls: Mutex<Vec<String>>,
}

impl Fake {
    fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }
}

impl OpenOps for Fake {
    async fn request_open(&self) -> Option<String> {
        self.calls.lock().unwrap().push("core".into());
        self.core.map(str::to_owned)
    }
    async fn ensure_home(&self, focus: bool) -> Result<(), String> {
        self.calls
            .lock()
            .unwrap()
            .push(format!("home focus={focus}"));
        self.ensure_fails.map_or(Ok(()), |e| Err(e.to_owned()))
    }
    async fn open_view(&self, split: bool) -> Result<(), String> {
        self.calls
            .lock()
            .unwrap()
            .push(format!("view split={split}"));
        if self.view_fails {
            Err("herdr down".into())
        } else {
            Ok(())
        }
    }
}

fn cfg(toml: &str) -> Map<String, Value> {
    toml.lines()
        .filter_map(|l| l.split_once(" = "))
        .map(|(k, v)| (k.to_owned(), serde_json::from_str(v).unwrap()))
        .collect()
}

async fn run_open(f: &Fake, c: &str, split: bool, popup: bool) -> (i32, String) {
    let mut err = vec![];
    let code = open(f, &cfg(c), OpenArgs { split, popup }, &mut err).await;
    (code, String::from_utf8(err).unwrap())
}

#[tokio::test]
async fn the_key_asks_the_core_first_in_per_tab_mode() {
    for did in ["panel", "home"] {
        let f = Fake {
            core: Some(did),
            ..Fake::default()
        };
        assert_eq!(
            run_open(&f, "cockpit = true", false, false).await,
            (0, String::new())
        );
        assert_eq!(f.calls(), ["core"], "{did}: the core did it");
    }
    // the core could do neither: said, then the home tab
    let f = Fake {
        core: Some("failed"),
        ..Fake::default()
    };
    let (code, err) = run_open(&f, "cockpit = true", false, false).await;
    assert_eq!(
        (code, err.as_str()),
        (0, "panorama: open: failed; showing the home tab\n")
    );
    assert_eq!(f.calls(), ["core", "home focus=true"]);
    // no core of ours: the home tab, quietly
    let f = Fake::default();
    assert_eq!(
        run_open(&f, "cockpit = true", false, false).await,
        (0, String::new())
    );
    assert_eq!(f.calls(), ["core", "home focus=true"]);
}

#[tokio::test]
async fn without_per_tab_panels_the_key_focuses_the_home_tab() {
    // test_home.py::test_open_action_focuses_home_and_popup_stays_available
    let f = Fake {
        core: Some("panel"),
        ..Fake::default()
    };
    assert_eq!(
        run_open(&f, "cockpit = false", false, false).await,
        (0, String::new())
    );
    assert_eq!(f.calls(), ["home focus=true"], "no core asked");
    let f = Fake::default();
    assert_eq!(
        run_open(&f, "cockpit = false", false, true).await,
        (0, String::new())
    );
    assert_eq!(
        f.calls(),
        ["view split=false"],
        "--popup: the popup even when the home tab is on"
    );
    let f = Fake::default();
    assert_eq!(
        run_open(&f, "cockpit = true", true, false).await,
        (0, String::new())
    );
    assert_eq!(
        f.calls(),
        ["view split=true"],
        "--split: straight to the split"
    );
}

#[tokio::test]
async fn the_home_tab_off_or_failing_means_the_popup() {
    // test_home.py::test_home_tab_off_means_popup_and_no_startup_tab, test_r5_h1 and
    // test_cockpit.py::test_r8_unverifiable_lookalikes_open_nothing: the key still works
    let f = Fake {
        core: Some("panel"),
        ..Fake::default()
    };
    assert_eq!(
        run_open(&f, "home_tab = false\ncockpit = true", false, false).await,
        (0, String::new())
    );
    assert_eq!(f.calls(), ["view split=false"]);
    let f = Fake {
        ensure_fails: Some("could not verify"),
        ..Fake::default()
    };
    let (code, err) = run_open(&f, "cockpit = false", false, false).await;
    assert_eq!(
        (code, err.as_str()),
        (
            0,
            "panorama: home tab: could not verify; opening the popup\n"
        )
    );
    assert_eq!(f.calls(), ["home focus=true", "view split=false"]);
    let f = Fake {
        ensure_fails: Some("x"),
        view_fails: true,
        ..Fake::default()
    };
    let (code, err) = run_open(&f, "cockpit = false", false, false).await;
    assert_eq!(code, 1);
    assert!(
        err.ends_with("panorama: could not open the view: herdr down\n"),
        "{err}"
    );
}

#[tokio::test]
async fn home_startup_opens_once_without_the_focus_and_nothing_when_off() {
    // test_home.py::test_startup_does_not_take_focus, test_home_tab_off_means_popup_and_no_startup_tab
    let mut err = vec![];
    let f = Fake::default();
    assert_eq!(home(&f, &cfg("cockpit = true"), true, &mut err).await, 0);
    assert_eq!(home(&f, &cfg("cockpit = true"), false, &mut err).await, 0);
    assert_eq!(f.calls(), ["home focus=false", "home focus=true"]);
    let f = Fake::default();
    assert_eq!(home(&f, &cfg("home_tab = false"), true, &mut err).await, 0);
    assert!(f.calls().is_empty());
    let f = Fake {
        ensure_fails: Some("no herdr"),
        ..Fake::default()
    };
    assert_eq!(home(&f, &cfg(""), true, &mut err).await, 1);
    assert_eq!(
        String::from_utf8(err).unwrap(),
        "panorama: home tab: no herdr\n"
    );
}

#[test]
fn the_flags_are_argparses() {
    let args = |a: &[&str]| a.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    let open = ["--split", "--popup"];
    assert_eq!(
        parse_flags(&args(&["--popup", "--split"]), &open),
        Ok(vec!["--popup", "--split"])
    );
    assert_eq!(parse_flags(&args(&[]), &open), Ok(vec![]));
    assert_eq!(
        parse_flags(&args(&["--startup"]), &open),
        Err("unrecognized arguments: --startup".into())
    );
    assert_eq!(main("open", &args(&["--bogus"])), 2);
    assert_eq!(main("home", &args(&["--split"])), 2);
}
