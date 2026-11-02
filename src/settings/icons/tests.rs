use super::*;
use serde_json::json;

fn config(value: Value) -> Map<String, Value> {
    json!({"harness_icons": value}).as_object().unwrap().clone()
}

#[test]
fn override_auto_and_invalid_settings_resolve_without_io() {
    for auto in [false, true] {
        assert_eq!(resolve(&config(json!("font")), auto), HarnessIcons::Font);
        assert_eq!(resolve(&config(json!("text")), auto), HarnessIcons::Text);
        for cfg in [
            Map::new(),
            config(json!("auto")),
            config(json!(false)),
            config(json!("typo")),
        ] {
            assert_eq!(
                resolve(&cfg, auto),
                if auto {
                    HarnessIcons::Font
                } else {
                    HarnessIcons::Text
                }
            );
        }
    }
}

fn environment(home: &Path, terminal: &str, ssh: bool) -> PluginEnv {
    let mut vars = vec![("TERM_PROGRAM", terminal)];
    if ssh {
        vars.push(("SSH_CONNECTION", "synthetic"));
    }
    PluginEnv::from_vars(vars, Some(home.to_path_buf()), 1000)
}

#[test]
fn auto_requires_local_ghostty_mapping_and_installed_font() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    let env = environment(home, "ghostty", false);
    let cfg = home.join(".config/ghostty");
    let fonts = home.join(".local/share/fonts");
    fs::create_dir_all(&cfg).unwrap();
    fs::create_dir_all(&fonts).unwrap();
    let mapping = "font-codepoint-map = U+E1A0-U+E1BA=Herdr Agent Icons Max\n";
    assert!(!detect(&env));
    fs::write(cfg.join("config.ghostty"), mapping).unwrap();
    assert!(!detect(&env));
    fs::write(
        fonts.join("HerdrAgentIconsMax-fixture.ttf"),
        b"synthetic font installation hint",
    )
    .unwrap();
    assert!(detect(&env));
    assert!(detect(&environment(home, "herdr", false)));
    assert!(!detect(&environment(home, "xterm", false)));
    assert!(!detect(&environment(home, "ghostty", true)));
    for bad in [
        format!("# {mapping}"),
        mapping.replace("E1BA", "E1A0"),
        mapping.replace(FAMILY, "another face"),
    ] {
        fs::write(cfg.join("config.ghostty"), bad).unwrap();
        assert!(!detect(&env));
    }
    fs::write(
        cfg.join("config.ghostty"),
        format!("{mapping}{}", " ".repeat(MAX_CONFIG as usize)),
    )
    .unwrap();
    assert!(!detect(&env));
}

#[test]
fn settings_cache_capability_but_re_read_explicit_mode() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    let cfg = home.join(".config/ghostty");
    let own = home.join("panorama");
    let fonts = home.join(".fonts");
    for path in [&cfg, &own, &fonts] {
        fs::create_dir_all(path).unwrap();
    }
    fs::write(
        cfg.join("config"),
        "font-codepoint-map=U+E1A0-U+E1BA=Herdr Agent Icons Max",
    )
    .unwrap();
    fs::write(fonts.join("HerdrAgentIconsMax.ttf"), b"fixture").unwrap();
    let env = PluginEnv::from_vars(
        [
            ("TERM_PROGRAM", "ghostty"),
            ("HERDR_PLUGIN_CONFIG_DIR", own.to_str().unwrap()),
        ],
        Some(home.to_path_buf()),
        1000,
    );
    let mut settings = crate::settings::Settings::from_env(&env);
    assert_eq!(settings.read().harness_icons, HarnessIcons::Font);
    fs::remove_file(cfg.join("config")).unwrap();
    assert_eq!(settings.read().harness_icons, HarnessIcons::Font);
    fs::write(own.join("config.toml"), "harness_icons = 'text'").unwrap();
    assert_eq!(settings.read().harness_icons, HarnessIcons::Text);
}
