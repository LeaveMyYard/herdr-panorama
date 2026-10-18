use super::*;
use std::fs;
use std::path::Path;

fn write(dir: &Path, text: &str) {
    fs::create_dir_all(dir).unwrap();
    let path = dir.join("config.toml");
    fs::write(&path, text).unwrap();
}

#[test]
fn both_files_are_read_and_re_read_when_they_change() {
    let tmp = tempfile::tempdir().unwrap();
    let (own, herdr) = (tmp.path().join("own"), tmp.path().join("herdr"));
    let mut s = Settings::new(Some(own.clone()), herdr.clone(), PANORAMA_ID);
    assert_eq!(s.read().panel, PanelSettings::default());
    assert_eq!(s.read().theme, Tokens::builtin("catppuccin"));
    assert_eq!(
        s.read().open_key,
        ("pfx+spc".to_owned(), "pfx+spc".to_owned())
    );
    write(&own, "panel_colors = \"herdr\"\nanimation_fps = 10\n");
    write(
        &herdr,
        "[theme]\nname = \"nord\"\n[[keys.command]]\nkey = \"ctrl+alt+p\"\ntype = \"plugin_action\"\ncommand = \"leavemyyard.herdr-panorama.open\"\n",
    );
    assert!(s.read().panel.follow_herdr);
    assert_eq!(s.read().animation_fps, 10.0);
    assert_eq!(s.read().theme, Tokens::builtin("nord"));
    assert_eq!(s.read().open_key.1, "c-a-p");
    write(
        &herdr,
        "[theme]\nname = \"terminal\"\n# a longer file: another size\n",
    );
    assert_eq!(s.read().theme, None);
}

#[test]
fn a_broken_file_reads_as_empty_and_says_why() {
    let tmp = tempfile::tempdir().unwrap();
    let herdr = tmp.path().join("herdr");
    write(&herdr, "not toml [[[");
    let mut s = Settings::new(None, herdr, PANORAMA_ID);
    assert_eq!(s.read().open_key.0, "pfx+spc");
    assert_eq!(s.read().theme, Tokens::builtin("catppuccin"));
    assert_eq!(s.errors().count(), 1);
    assert_eq!(s.read().panel, PanelSettings::default()); // no own config dir: defaults
}
