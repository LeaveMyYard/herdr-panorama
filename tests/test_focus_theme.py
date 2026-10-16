"""Design turn 10 (docs/design/swarm-focus.js, approved by the operator as drawn): the docked 9e
panel shows whether panorama's pane has the keyboard, in herdr's selected theme. Focused, the
theme's active_row_bg; not focused, its panel_bg with the secondary text one step dimmer; tiles,
status colours and pulses as in 9e. Golden: render-10.mjs -> tests/fixtures/design_10.json.
Also: herdr's config.toml is read as herdr layers it, and re-read when it changes."""

import json
import os

import pytest

from herdr_panorama import render, theme

from .design_dock import FIX, design_swarm

TEN = json.loads((FIX / "design_10.json").read_text())
NINE = json.loads((FIX / "design_9.json").read_text())


def norm(ch, fg, bg, bold):
    bg = (bg or render.BG).lower()
    return (ch, None, bg, None) if ch == " " else (ch, (fg or render.FG).lower(), bg, bool(bold))


def grid(runs):
    return [[norm(ch, r[1], r[2], r[3]) for r in row for ch in r[0]] for row in runs]


def panel(frame, tokens, focused):
    v = render.View(orch="w1:p3", dock=True, frame=frame)
    v.hover = ("orch", ("w1:p4",))  # the design's hover
    v.panel = theme.panel_colors(tokens, focused, render.FG, render.DIM, render.FAINT, (render.DOCK_HOV, render.BG))
    scr = render.render(design_swarm(NINE["data"]), v, 46, 45)
    return [[norm(c.ch, c.fg, c.bg, c.bold) for c in scr.c[y][:46]] for y in range(45)]


CASES = [(f, key, mode) for f in sorted(TEN["frames"], key=int) for key in ("yours", "tokyo") for mode in "AI"]


@pytest.mark.parametrize("frame,key,mode", CASES)
def test_turn_10_matches_the_design_cell_for_cell(frame, key, mode):
    want = grid(TEN["frames"][frame][key + mode])
    got = panel(int(frame), TEN["themes"][key], mode == "A")
    d = [(x, y, want[y][x], got[y][x]) for y in range(45) for x in range(46) if want[y][x] != got[y][x]]
    assert not d, f"{len(d)} cells differ, first: {d[:5]}"


def test_the_design_uses_our_theme_table():
    assert TEN["themes"]["tokyo"] == theme.THEMES["tokyo-night"]
    assert TEN["themes"]["yours"] == {**theme.THEMES["catppuccin"], "active_row_bg": "#414868"}


def write(tmp_path, monkeypatch, text):
    monkeypatch.setenv("XDG_CONFIG_HOME", str(tmp_path))
    p = tmp_path / "herdr" / "config.toml"
    p.parent.mkdir(parents=True, exist_ok=True)
    p.write_text(text)
    n = 1_000_000 + len(text) + int(os.urandom(2).hex(), 16)
    os.utime(p, (n, n))  # a fresh mtime: the cache re-reads
    return p


def test_the_operators_config_gives_the_designs_colours(tmp_path, monkeypatch):
    write(tmp_path, monkeypatch, '[theme.custom]\nactive_row_bg = "#414868"\n')  # no name: catppuccin
    assert theme.current() == TEN["themes"]["yours"]


def test_layers_name_custom_then_dark_with_auto_switch(tmp_path, monkeypatch):
    write(tmp_path, monkeypatch, '[theme]\nname = "gruvbox"\nauto_switch = true\ndark_name = "nord"\n'
                                 '[theme.custom]\ntext = "rgb(1,2,3)"\npanel_bg = "Red"\n'
                                 '[theme.custom.dark]\ntext = "#ABCDEF"\n')
    t = theme.current()
    assert t["active_row_bg"] == theme.THEMES["nord"]["active_row_bg"]  # dark_name with auto_switch
    assert t["panel_bg"] == "#ff0000" and t["text"] == "#abcdef"  # custom, then custom.dark on top


def test_reset_and_unknown_values(tmp_path, monkeypatch):
    write(tmp_path, monkeypatch, '[theme]\nname = "nope"\n[theme.custom]\npanel_bg = "reset"\ntext = "chartreuse-ish"\n')
    t = theme.current()
    assert t["panel_bg"] is None and t["text"] == theme.THEMES["catppuccin"]["text"]  # unknown: herdr's fallback
    bg, _ = theme.panel_colors(t, False, render.FG, render.DIM, render.FAINT, (render.DOCK_HOV, render.BG))
    assert bg == render.BG  # reset: panorama's own background


def test_the_terminal_theme_keeps_panoramas_colours_with_a_focus_cue(tmp_path, monkeypatch):
    write(tmp_path, monkeypatch, '[theme]\nname = "terminal"\n')
    assert theme.current() is None
    fb = (render.DOCK_HOV, render.BG)
    assert theme.panel_colors(None, True, render.FG, render.DIM, render.FAINT, fb) == (render.DOCK_HOV, {})
    assert theme.panel_colors(None, False, render.FG, render.DIM, render.FAINT, fb) == (render.BG, {})


def test_a_changed_file_is_read_again(tmp_path, monkeypatch):
    write(tmp_path, monkeypatch, '[theme]\nname = "nord"\n')
    assert theme.current()["panel_bg"] == theme.THEMES["nord"]["panel_bg"]
    write(tmp_path, monkeypatch, '[theme]\nname = "vesper"\n')
    assert theme.current()["panel_bg"] == theme.THEMES["vesper"]["panel_bg"]


def test_every_built_in_theme_has_the_six_tokens():
    names = ("catppuccin catppuccin-latte tokyo-night tokyo-night-day dracula nord gruvbox gruvbox-light one-dark "
             "one-light solarized solarized-light kanagawa kanagawa-lotus rose-pine rose-pine-dawn vesper").split()
    assert sorted(theme.THEMES) == sorted(names)  # herdr 0.9.3's list, less "terminal"
    for t in theme.THEMES.values():
        assert set(t) == set(theme.TOKENS) and all(theme.color(v) == v for v in t.values())


async def test_the_view_follows_the_terminals_focus_in_and_out(fake_herdr):
    """Textual's app focus / blur (the terminal's focus reports, which herdr passes to the
    focused pane) switch the docked panel between the two looks; the 2a graph is unchanged."""
    from textual import events

    from herdr_panorama.tui import SwarmApp
    app = SwarmApp(source=lambda: design_swarm(NINE["data"]), live_refresh=False, clock=lambda: 0.0,
                   focus_pane="w1:p3", home_mode=True, cockpit_mode=True)
    cat = theme.THEMES["catppuccin"]  # no herdr config: herdr's default theme
    async with app.run_test(size=(46, 45)) as pilot:
        for _ in range(50):
            await pilot.pause(0.02)
            if app.loaded:
                break
        bg = lambda: app.grid.screen_data.c[30][20].bg  # noqa: E731 - an empty row of the panel
        assert app.view.dock and bg() == cat["active_row_bg"]
        app.post_message(events.AppBlur())
        await pilot.pause()
        assert bg() == cat["panel_bg"]
        app.post_message(events.AppFocus())
        await pilot.pause()
        assert bg() == cat["active_row_bg"]
    wide = SwarmApp(source=lambda: design_swarm(NINE["data"]), live_refresh=False, clock=lambda: 0.0,
                    focus_pane="w1:p3", home_mode=True, cockpit_mode=True)
    async with wide.run_test(size=(160, 45)) as pilot:
        for _ in range(50):
            await pilot.pause(0.02)
            if wide.loaded:
                break
        assert not wide.view.dock and wide.view.panel is None
        wide.post_message(events.AppBlur())
        await pilot.pause()
        assert wide.view.panel is None and wide.grid.screen_data.c[44][100].bg in (None, render.BG)


def test_the_table_uses_what_herdr_draws_where_it_differs_from_the_palettes():
    """Review of #19: checked against herdr 0.9.3's own output (an isolated herdr per theme, a
    client attached). These are herdr's colours, not the public palettes' near ones."""
    assert theme.HERDR_VERSION == "0.9.3"
    measured = {("tokyo-night", "overlay1"): "#697196", ("nord", "overlay1"): "#646e82",
                ("one-dark", "subtext0"): "#969ca8", ("one-dark", "overlay1"): "#737a87",
                ("one-light", "overlay1"): "#686b77", ("dracula", "subtext0"): "#d2d2dc",
                ("dracula", "overlay0"): "#828cb4", ("kanagawa", "subtext0"): "#c8c3aa",
                ("kanagawa", "overlay0"): "#87867d", ("rose-pine-dawn", "text"): "#464261",
                ("vesper", "overlay1"): "#7e7e7e", ("vesper", "overlay0"): "#5c5c5c"}
    assert {k: theme.THEMES[k[0]][k[1]] for k in measured} == measured
