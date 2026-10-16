"""herdr's selected theme, for the docked panel's focused / not-focused look (design turn 10,
``docs/design/swarm-focus.js``; the operator approved it as drawn).

herdr's API does not expose theme colours, so they are read from herdr's own ``config.toml``
(``$XDG_CONFIG_HOME/herdr``, else ``~/.config/herdr``): ``[theme] name`` (default "catppuccin";
with ``auto_switch``, its ``dark_name``, since panorama's own colours are dark), then the
``[theme.custom]`` overrides (and ``[theme.custom.dark]`` with ``auto_switch``). Re-read when
the file changes (``herdr server reload-config`` follows an edit to it).

The built-in themes' tokens below are ours, from each theme's public palette: only the six the
panel reads. ``panel_colors`` is the one place that turns them into the panel's colours.
"""

from __future__ import annotations

import logging
import os
import re
import tomllib
from pathlib import Path

log = logging.getLogger("herdr_panorama.theme")

TOKENS = ("panel_bg", "active_row_bg", "text", "subtext0", "overlay1", "overlay0")


def _t(panel_bg, active_row_bg, text, subtext0, overlay1, overlay0) -> dict:
    return dict(zip(TOKENS, (panel_bg, active_row_bg, text, subtext0, overlay1, overlay0), strict=True))


# name -> (panel_bg, active_row_bg, text, subtext0, overlay1, overlay0), from the public palettes
THEMES: dict[str, dict] = {
    "catppuccin": _t("#1e1e2e", "#313244", "#cdd6f4", "#a6adc8", "#7f849c", "#6c7086"),  # mocha
    "catppuccin-latte": _t("#eff1f5", "#ccd0da", "#4c4f69", "#6c6f85", "#8c8fa1", "#9ca0b0"),
    "tokyo-night": _t("#1a1b26", "#292e42", "#c0caf5", "#a9b1d6", "#737aa2", "#565f89"),
    "tokyo-night-day": _t("#e1e2e7", "#c4c8da", "#3760bf", "#6172b0", "#68709a", "#848cb5"),
    "dracula": _t("#282a36", "#44475a", "#f8f8f2", "#bfbfbf", "#6272a4", "#525a7d"),
    "nord": _t("#2e3440", "#3b4252", "#eceff4", "#d8dee9", "#616e88", "#4c566a"),
    "gruvbox": _t("#282828", "#3c3836", "#ebdbb2", "#d5c4a1", "#928374", "#7c6f64"),
    "gruvbox-light": _t("#fbf1c7", "#ebdbb2", "#3c3836", "#504945", "#928374", "#a89984"),
    "one-dark": _t("#282c34", "#3e4451", "#abb2bf", "#9da5b4", "#7f848e", "#5c6370"),
    "one-light": _t("#fafafa", "#e5e5e6", "#383a42", "#4f525e", "#696c77", "#a0a1a7"),
    "solarized": _t("#002b36", "#073642", "#93a1a1", "#839496", "#657b83", "#586e75"),
    "solarized-light": _t("#fdf6e3", "#eee8d5", "#586e75", "#657b83", "#839496", "#93a1a1"),
    "kanagawa": _t("#1f1f28", "#363646", "#dcd7ba", "#c8c093", "#727169", "#54546d"),
    "kanagawa-lotus": _t("#f2ecbc", "#e4d794", "#545464", "#716e61", "#8a8980", "#a09cac"),
    "rose-pine": _t("#191724", "#403d52", "#e0def4", "#908caa", "#6e6a86", "#524f67"),
    "rose-pine-dawn": _t("#faf4ed", "#dfdad9", "#575279", "#797593", "#9893a5", "#cecacd"),
    "vesper": _t("#101010", "#232323", "#ffffff", "#a0a0a0", "#8b8b8b", "#505050"),
    # "terminal" follows the host terminal's ANSI palette, which panorama cannot read: none
}
DEFAULT = "catppuccin"
RESET = frozenset({"reset", "default", "none", "transparent"})
NAMED = {"black": "#000000", "white": "#ffffff", "red": "#ff0000", "green": "#00ff00", "blue": "#0000ff",
         "yellow": "#ffff00", "cyan": "#00ffff", "magenta": "#ff00ff", "gray": "#808080", "grey": "#808080"}
_HEX = re.compile(r"^#([0-9a-fA-F]{6})$")
_RGB = re.compile(r"^rgb\(\s*(\d{1,3})\s*,\s*(\d{1,3})\s*,\s*(\d{1,3})\s*\)$")

_cache: dict = {"path": None, "mtime": None, "tokens": None}


def config_path() -> Path:
    base = os.environ.get("XDG_CONFIG_HOME") or os.path.join(os.path.expanduser("~"), ".config")
    return Path(base) / "herdr" / "config.toml"


def color(value) -> str | None | bool:
    """A herdr colour value as ``#rrggbb``; None for a reset alias; False if it is not one."""
    if not isinstance(value, str):
        return False
    v = value.strip().lower()
    if v in RESET:
        return None
    if _HEX.match(v):
        return v
    m = _RGB.match(v)
    if m and all(int(g) <= 255 for g in m.groups()):
        return "#" + "".join(f"{int(g):02x}" for g in m.groups())
    return NAMED.get(v, False)


def resolve(cfg: dict) -> dict | None:
    """The six tokens for this herdr config: the built-in theme, then [theme.custom], then
    [theme.custom.dark] with auto_switch. None for the "terminal" theme (no palette to read)."""
    th = cfg.get("theme") if isinstance(cfg.get("theme"), dict) else {}
    auto = th.get("auto_switch") is True
    name = str((th.get("dark_name") if auto and th.get("dark_name") else th.get("name")) or DEFAULT).lower()
    if name == "terminal":
        return None
    tokens = dict(THEMES.get(name) or THEMES[DEFAULT])
    custom = th.get("custom") if isinstance(th.get("custom"), dict) else {}
    layers = [custom] + ([custom["dark"]] if auto and isinstance(custom.get("dark"), dict) else [])
    for layer in layers:
        for k in TOKENS:
            if k in layer:
                c = color(layer[k])
                if c is not False:
                    tokens[k] = c
    return tokens


def current() -> dict | None:
    """The tokens of herdr's theme now (re-read when its config.toml changes)."""
    path = config_path()
    try:
        mtime = path.stat().st_mtime
    except OSError:
        mtime = None
    if (path, mtime) != (_cache["path"], _cache["mtime"]):
        cfg: dict = {}
        if mtime is not None:
            try:
                with open(path, "rb") as fh:
                    cfg = tomllib.load(fh)
            except (OSError, tomllib.TOMLDecodeError) as exc:
                log.warning("ignoring herdr's %s: %s", path, exc)
        _cache.update(path=path, mtime=mtime, tokens=resolve(cfg))
    return _cache["tokens"]


def panel_colors(tokens: dict | None, focused: bool, fg: str, dim: str, faint: str,
                 fallback: tuple[str, str]) -> tuple[str, dict[str, str]]:
    """The docked panel's background and its text colours (panorama's FG, DIM, FAINT mapped)
    for herdr's theme, focused or not (design turn 10):

    - focused: the theme's active_row_bg; text, dim -> subtext0, faint -> overlay1
    - not focused: its panel_bg; text, dim -> overlay1, faint -> overlay0 (one step dimmer)

    A token that is unset or "reset" falls back to panorama's own colour; ``fallback`` is
    (focused, not focused) background for the "terminal" theme and a reset background."""
    if tokens is None:
        return fallback[0] if focused else fallback[1], {}
    bg = tokens["active_row_bg"] if focused else tokens["panel_bg"]
    pairs = {fg: tokens["text"], dim: tokens["subtext0"] if focused else tokens["overlay1"],
             faint: tokens["overlay1"] if focused else tokens["overlay0"]}
    return (bg or (fallback[0] if focused else fallback[1])), {k: v for k, v in pairs.items() if v}
