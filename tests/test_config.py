"""Plugin config: done_window from config.toml, env override, durations."""

from herdr_panorama import config


def test_default_is_one_hour(tmp_path, monkeypatch):
    assert config.done_window() == 3600


def test_config_file_and_env_override(tmp_path, monkeypatch):
    d = tmp_path / "plugin-config"
    d.mkdir(exist_ok=True)
    (d / "config.toml").write_text('done_window = "2h30m"\n')
    assert config.done_window() == 9000
    (d / "config.toml").write_text("done_window = 600\n")
    import os
    os.utime(d / "config.toml", (1, 1))  # a new mtime: re-read
    assert config.done_window() == 600
    monkeypatch.setenv("PANORAMA_DONE_WINDOW", "45m")
    assert config.done_window() == 2700


def test_bad_values_fall_back(tmp_path):
    d = tmp_path / "plugin-config"
    d.mkdir(exist_ok=True)
    (d / "config.toml").write_text("done_window = [\n")  # not TOML
    assert config.done_window() == 3600
    assert config.parse_duration("soon") is None and config.parse_duration(-5) is None
    assert config.parse_duration("90m") == 5400 and config.parse_duration("15") == 15


def test_animation_fps_defaults_to_20_and_is_clamped(tmp_path):
    import os
    d = tmp_path / "plugin-config"
    d.mkdir(exist_ok=True)
    assert config.animation_fps() == 20
    for i, (text, want) in enumerate((("animation_fps = 10", 10), ("animation_fps = 1", 2), ("animation_fps = 120", 30),
                                      ("animation_fps = 7.5", 7.5), ('animation_fps = "fast"', 20),
                                      ("animation_fps = true", 20))):
        (d / "config.toml").write_text(text + "\n")
        os.utime(d / "config.toml", (i + 1, i + 1))  # a new mtime: re-read
        assert config.animation_fps() == want, text
