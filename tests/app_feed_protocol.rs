mod app_feed_support;
use app_feed_support::Installed;
use panorama::app_feed::{
    fs::{self, Home},
    json,
    receipt::{Ownership, RECEIPT, Receipt, SMALL},
};
use serde_json::{Value, json as value};

#[test]
fn shared_lifecycle_fixture_parses_exact_fields_and_hashes() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/app-feed-runtime/lifecycle.json")).unwrap();
    assert_eq!(
        fixture["binary_entry"]["sha256"],
        fs::digest(fixture["binary_fixture_bytes"].as_str().unwrap().as_bytes())
    );
    assert_eq!(
        fixture["committed_ownership"]["feed_sha256"],
        fs::digest(include_bytes!("fixtures/app-feed/degraded.json"))
    );
    assert_eq!(
        fixture["generation_fence"]["install_id"],
        fixture["install_id"]
    );
    let generation = fixture["install_id"].as_str().unwrap();
    for key in ["ensure_args", "inbox_available_args", "inbox_open_args"] {
        let args = fixture["generation_fence"][key].as_array().unwrap();
        assert_eq!(args[args.len() - 2], "--install-id");
        assert_eq!(args.last().unwrap(), generation);
    }
    assert_ne!(
        fixture["generation_fence"]["replacement_install_id"],
        fixture["install_id"]
    );
    assert_eq!(
        fixture["empty_mod_data_dirs"],
        value!([
            "data/panorama-spaces",
            "data/panorama-graph",
            "data/panorama-peek",
            "data/panorama-tabs",
            "data/panorama-pane"
        ])
    );
    let installed = Installed::new();
    let home = Home::open(&installed.app).unwrap();
    let receipt = Receipt::read(&home).unwrap();
    for key in [
        "initial_ownership",
        "prepared_ownership",
        "committed_ownership",
    ] {
        let mut owner = fixture[key].clone();
        owner["binary_sha256"] = value!(receipt.binary_sha);
        let old = home
            .read(&receipt.path("ownership.json"), SMALL, 0o600)
            .unwrap();
        home.replace(
            &receipt.path("ownership.json"),
            &receipt.path(".ownership.tmp"),
            &serde_json::to_vec(&owner).unwrap(),
            old.as_ref(),
        )
        .unwrap();
        assert!(Ownership::read(&home, &receipt).unwrap().is_some());
    }
}

#[test]
fn runtime_json_rejects_duplicates_and_unsafe_tokens_without_touching_state() {
    assert!(json::parse(br#"{"v":1,"nested":{"x":1,"x":2}}"#).is_err());
    assert_eq!(json::parse(br#"{"v":1e0}"#).unwrap()["v"], 1);
    assert!(!json::token(&"x".repeat(257)));
    assert!(!json::token("start\n"));
    let installed = Installed::new();
    let home = Home::open(&installed.app).unwrap();
    let receipt = Receipt::read(&home).unwrap();
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/app-feed-runtime/lifecycle.json")).unwrap();
    let mut owner = fixture["initial_ownership"].clone();
    owner["binary_sha256"] = value!(receipt.binary_sha);
    for (key, bad) in [
        ("install_id", value!("ffffffffffffffffffffffffffffffff")),
        ("writer_start", value!("x".repeat(257))),
        ("writer_pid", value!(true)),
        ("pending_feed_sha256", value!("bad")),
    ] {
        let mut bad_owner = owner.clone();
        bad_owner[key] = bad;
        let old = home
            .read(&receipt.path("ownership.json"), SMALL, 0o600)
            .unwrap();
        home.replace(
            &receipt.path("ownership.json"),
            &receipt.path(".ownership.tmp"),
            &serde_json::to_vec(&bad_owner).unwrap(),
            old.as_ref(),
        )
        .unwrap();
        assert!(Ownership::read(&home, &receipt).is_err(), "{key}");
    }
    let mut bad_receipt: Value =
        serde_json::from_slice(&std::fs::read(installed.app.join(RECEIPT)).unwrap()).unwrap();
    bad_receipt["files"]["data/panorama/runtime/personal"] =
        value!({"sha256":"0".repeat(64),"mode":420});
    std::fs::write(
        installed.app.join(RECEIPT),
        serde_json::to_vec(&bad_receipt).unwrap(),
    )
    .unwrap();
    assert!(Receipt::read(&home).is_err());
}
