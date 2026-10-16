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


FB = (render.DOCK_HOV, render.BG)
HERDR = {"panel_colors": "herdr"}


def panel(frame, tokens, focused, cfg):
    v = render.View(orch="w1:p3", dock=True, frame=frame)
    v.hover = ("orch", ("w1:p4",))  # the design's hover
    v.panel = theme.panel_look(tokens, focused, render.FG, render.DIM, render.FAINT, FB, cfg)
    scr = render.render(design_swarm(NINE["data"]), v, 46, 45)
    return [[norm(c.ch, c.fg, c.bg, c.bold) for c in scr.c[y][:46]] for y in range(45)]


CASES = [(f, key, mode) for f in sorted(TEN["frames"], key=int) for key in ("yours", "tokyo") for mode in "AI"]


@pytest.mark.parametrize("frame,key,mode", CASES)
def test_turn_10_matches_the_design_cell_for_cell(frame, key, mode):
    want = grid(TEN["frames"][frame][key + mode])
    # "yours": the fixed default (no config); "tokyo": panel_colors = "herdr" with tokyo-night
    got = panel(int(frame), theme.THEMES["tokyo-night"] if key == "tokyo" else None, mode == "A",
                HERDR if key == "tokyo" else {})
    d = [(x, y, want[y][x], got[y][x]) for y in range(45) for x in range(46) if want[y][x] != got[y][x]]
    assert not d, f"{len(d)} cells differ, first: {d[:5]}"


def test_the_design_uses_our_theme_table():
    assert TEN["themes"]["tokyo"] == theme.THEMES["tokyo-night"]
    assert TEN["themes"]["yours"] == {"fixed": True, "focus_bg": theme.FIXED[0], "panel_bg": theme.FIXED[1]}
    assert theme.FIXED == ("#14161b", "#101215")  # the operator's colours


def write(tmp_path, monkeypatch, text):
    monkeypatch.setenv("XDG_CONFIG_HOME", str(tmp_path))
    p = tmp_path / "herdr" / "config.toml"
    p.parent.mkdir(parents=True, exist_ok=True)
    p.write_text(text)
    n = 1_000_000 + len(text) + int(os.urandom(2).hex(), 16)
    os.utime(p, (n, n))  # a fresh mtime: the cache re-reads
    return p


def test_herdrs_custom_layer_overrides_the_built_in_theme(tmp_path, monkeypatch):
    write(tmp_path, monkeypatch, '[theme.custom]\nactive_row_bg = "#414868"\n')  # no name: catppuccin
    assert theme.current() == {**theme.THEMES["catppuccin"], "active_row_bg": "#414868"}


def test_fixed_is_the_default_and_panoramas_own_colours_replace_either():
    assert theme.mode({}) == "fixed" and theme.mode(HERDR) == "herdr"
    cat = theme.THEMES["catppuccin"]
    assert theme.panel_colors(cat, True, render.FG, render.DIM, render.FAINT, FB, {}) == ("#14161b", {})
    assert theme.panel_colors(cat, False, render.FG, render.DIM, render.FAINT, FB, {}) == ("#101215", {})
    own = {"panel_bg": "#000001", "focus_bg": "#000002"}
    assert theme.panel_colors(cat, True, render.FG, render.DIM, render.FAINT, FB, own)[0] == "#000002"
    assert theme.panel_colors(cat, False, render.FG, render.DIM, render.FAINT, FB, own)[0] == "#000001"
    herdr_own = {**HERDR, **own}  # in the herdr mode too, with the theme's text colours
    bg, fgs = theme.panel_colors(cat, True, render.FG, render.DIM, render.FAINT, FB, herdr_own)
    assert bg == "#000002" and fgs[render.FG] == cat["text"]
    assert theme.own_colors({"panel_bg": "nope"}) == (None, None)  # not a colour: ignored


def test_the_herdr_mode_tints_toward_active_row_bg():
    cat = {**theme.THEMES["catppuccin"], "panel_bg": "#181825", "active_row_bg": "#414868"}
    focused = lambda cfg: theme.panel_colors(cat, True, render.FG, render.DIM, render.FAINT, FB, cfg)[0]  # noqa: E731
    assert focused(HERDR) == "#1e1f2f"  # the default 0.15
    assert focused({**HERDR, "focus_tint": 0.3}) == "#242639"
    assert focused({**HERDR, "focus_tint": 0}) == "#181825" and focused({**HERDR, "focus_tint": 1}) == "#414868"
    assert focused({**HERDR, "focus_tint": "x"}) == "#1e1f2f" and focused({**HERDR, "focus_tint": 5}) == "#414868"


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
    bg, _ = theme.panel_colors(t, False, render.FG, render.DIM, render.FAINT, FB, HERDR)
    assert bg == render.BG  # reset: panorama's own background


def test_the_terminal_theme_keeps_panoramas_colours_with_a_focus_cue(tmp_path, monkeypatch):
    write(tmp_path, monkeypatch, '[theme]\nname = "terminal"\n')
    assert theme.current() is None
    assert theme.panel_colors(None, True, render.FG, render.DIM, render.FAINT, FB, HERDR) == (
        theme._mix(render.BG, render.DOCK_HOV, theme.DEFAULT_TINT), {})
    assert theme.panel_colors(None, False, render.FG, render.DIM, render.FAINT, FB, HERDR) == (render.BG, {})


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
    focused, unfocused = theme.FIXED  # no config: the fixed default
    async with app.run_test(size=(46, 45)) as pilot:
        for _ in range(50):
            await pilot.pause(0.02)
            if app.loaded:
                break
        bg = lambda: app.grid.screen_data.c[30][20].bg  # noqa: E731 - an empty row of the panel
        assert app.view.dock and bg() == focused
        app.post_message(events.AppBlur())
        await pilot.pause()
        assert bg() == unfocused
        app.post_message(events.AppFocus())
        await pilot.pause()
        assert bg() == focused
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


def test_the_table_is_herdrs_measured_values():
    """Review of #19: herdr 0.9.3's own values, measured per token (sentinel overrides), for
    the entries the palettes had wrong (the last nine at cell level); none is unverified."""
    assert theme.HERDR_VERSION == "0.9.3"
    measured = {
        "catppuccin": {"panel_bg": "#181825", "active_row_bg": "#1e1e2e"},
        "catppuccin-latte": {"active_row_bg": "#e6e9ef"}, "tokyo-night": {"active_row_bg": "#232636"}, "tokyo-night-day": {"active_row_bg": "#d2d3da", "overlay0": "#8990b3"},
        "dracula": {"active_row_bg": "#373c52", "subtext0": "#d2d2dc", "overlay1": "#828cb4", "overlay0": "#6272a4"},
        "nord": {"active_row_bg": "#434c5e", "overlay1": "#646e82"},
        "gruvbox": {"active_row_bg": "#323130", "overlay1": "#a89984", "overlay0": "#928374"},
        "gruvbox-light": {"active_row_bg": "#f2e5bc", "overlay1": "#7c6f64", "overlay0": "#928374"},
        "one-dark": {"active_row_bg": "#313640", "subtext0": "#969ca8", "overlay1": "#737a87"},
        "one-light": {"active_row_bg": "#d8dbe2", "subtext0": "#686b77"}, "solarized": {"active_row_bg": "#164b57"},
        "solarized-light": {"text": "#657b83", "subtext0": "#839496", "overlay1": "#586e75"},
        "kanagawa": {"subtext0": "#c8c3aa", "overlay1": "#87867d", "overlay0": "#727169"},
        "kanagawa-lotus": {"active_row_bg": "#d5cea3", "subtext0": "#43436c"},
        "rose-pine": {"active_row_bg": "#26233a", "subtext0": "#c8c5dc", "overlay1": "#908caa", "overlay0": "#6e6a86"},
        "rose-pine-dawn": {"active_row_bg": "#e3d9cf", "text": "#464261", "overlay1": "#797593", "overlay0": "#9893a5"},
        "vesper": {"panel_bg": "#1a1a1a", "active_row_bg": "#101010", "overlay1": "#7e7e7e", "overlay0": "#5c5c5c"},
    }
    got = {name: {k: theme.THEMES[name][k] for k in toks} for name, toks in measured.items()}
    assert got == measured
    assert theme.UNVERIFIED == set()  # every token measured


async def test_herdrs_focus_events_switch_the_look_too(fake_herdr):
    """The operator's report: the open key (tab.focus + pane.focus on panorama) focused the panel
    but it kept the unfocused look; the terminal's focus report did not arrive. herdr's own
    pane_focused events now count: panorama's pane focused -> focused, another pane -> not."""
    from herdr_panorama.tui import SwarmApp
    app = SwarmApp(source=lambda: design_swarm(NINE["data"]), live_refresh=False, clock=lambda: 0.0,
                   focus_pane="w1:p3", home_mode=True, cockpit_mode=True)
    focused, unfocused = theme.FIXED
    async with app.run_test(size=(46, 45)) as pilot:
        for _ in range(50):
            await pilot.pause(0.02)
            if app.loaded:
                break
        app.my_pane = "w1:p9"  # panorama's own pane
        bg = lambda: app.grid.screen_data.c[30][20].bg  # noqa: E731
        app.herdr_focus("w1:p3")  # the agent got the keyboard
        await pilot.pause()
        assert not app.pane_focused and bg() == unfocused
        app.herdr_focus("w1:p9")  # the open key: herdr focuses panorama's pane
        await pilot.pause()
        assert app.pane_focused and bg() == focused


async def test_the_focus_watcher_forwards_herdrs_pane_focus(fake_herdr, monkeypatch):
    """The event stream itself: a pane_focused for panorama's pane reaches herdr_focus."""
    from herdr_panorama import herdr
    from herdr_panorama.tui import SwarmApp
    app = SwarmApp(source=lambda: design_swarm(NINE["data"]), live_refresh=False, clock=lambda: 0.0,
                   focus_pane="w1:p3", home_mode=True, cockpit_mode=True)
    seen = []
    monkeypatch.setattr(app, "herdr_focus", lambda pane: seen.append(pane))
    monkeypatch.setattr(app, "call_from_thread", lambda fn, *a: fn(*a))
    monkeypatch.setattr(app, "focus_event", lambda: None)
    monkeypatch.setattr(herdr, "events", lambda kinds, stop: iter([
        {"type": "_connected"}, {"type": "tab_focused", "tab_id": "w1:t3"},
        {"type": "pane_focused", "pane_id": "w1:p9"}]))
    app._watch_focus()
    assert seen == ["w1:p9"]


def test_the_hover_row_is_a_step_lighter_than_either_background():
    """#25: the fixed hover #171a1f vanished into the focused #14161b. It now follows the panel."""
    assert theme.hover_bg("#14161b") == "#1d1f24" and theme.hover_bg("#101215") == "#1a1b1e"
    for focused in (True, False):
        bg, _, hover = theme.panel_look(None, focused, render.FG, render.DIM, render.FAINT, FB, {})
        assert hover == theme.hover_bg(bg) and hover != bg and hover != render.DOCK_HOV


async def test_a_panorama_started_without_the_keyboard_looks_unfocused(fake_herdr, monkeypatch):
    """Review of #25: the startup hook opens panorama without focus and terminals report focus
    only on change. When the focus stream connects, herdr's snapshot decides."""
    from herdr_panorama import herdr
    from herdr_panorama.tui import SwarmApp
    app = SwarmApp(source=lambda: design_swarm(NINE["data"]), live_refresh=False, clock=lambda: 0.0,
                   focus_pane="w1:p3", home_mode=True, cockpit_mode=True)
    app.my_pane = "w1:p9"
    seen = []
    monkeypatch.setattr(app, "call_from_thread", lambda fn, *a: (seen.append(a), fn(*a)))
    monkeypatch.setattr(app, "focus_event", lambda: None)
    monkeypatch.setattr(app, "repaint", lambda: None)
    monkeypatch.setattr(herdr, "snapshot", lambda: {"focused_pane_id": "w1:p3"})  # the agent has it
    monkeypatch.setattr(herdr, "events", lambda kinds, stop: iter([{"type": "_connected"}]))
    assert app.pane_focused  # the initial guess
    app._watch_focus()
    assert seen == [("w1:p3",)] and not app.pane_focused


async def test_a_new_pane_id_after_a_dock_is_judged_again(fake_herdr):
    """Review of #25: a dock across workspaces gives panorama a new pane id; herdr's focus event
    for it arrives before the app knows the id. When the id changes, the last event counts."""
    from herdr_panorama.tui import SwarmApp
    app = SwarmApp(source=lambda: design_swarm(NINE["data"]), live_refresh=False, clock=lambda: 0.0,
                   focus_pane="w1:p3", home_mode=True, cockpit_mode=True)
    focused, unfocused = theme.FIXED
    async with app.run_test(size=(46, 45)) as pilot:
        for _ in range(50):
            await pilot.pause(0.02)
            if app.loaded:
                break
        bg = lambda: app.grid.screen_data.c[30][20].bg  # noqa: E731
        app.my_pane = "w5:p1"
        app.herdr_focus("w7:p2")  # the new id, before the dock worker returned it
        await pilot.pause()
        assert not app.pane_focused and bg() == unfocused
        app.my_pane = "w7:p2"  # the dock worker's result
        app._docked(None)
        await pilot.pause()
        assert app.pane_focused and bg() == focused
        app.my_pane = "w8:p3"
        app._followed("w8:p3", "docked")  # a follow that changed the id: judged again too
        await pilot.pause()
        assert not app.pane_focused
