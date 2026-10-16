"""herdr's selected theme, for the docked panel's focused / not-focused look (design turn 10,
``docs/design/swarm-focus.js``; the operator approved it as drawn).

herdr's API does not expose theme colours, so they are read from herdr's own ``config.toml``
(``$XDG_CONFIG_HOME/herdr``, else ``~/.config/herdr``): ``[theme] name`` (default "catppuccin";
with ``auto_switch``, its ``dark_name``, since panorama's own colours are dark), then the
``[theme.custom]`` overrides (and ``[theme.custom.dark]`` with ``auto_switch``). Re-read when
the file changes (``herdr server reload-config`` follows an edit to it).

The built-in themes' tokens below are herdr 0.9.3's own, measured (review of #19): an isolated
herdr per theme with a client attached, rendered plain and once per token with that token
overridden by a sentinel colour; the colour that disappears is herdr's value (the last few, that
share a colour with another token, at cell level). Only the six the panel reads. ``panel_colors`` is the one place that turns them into the panel's colours.
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


HERDR_VERSION = "0.9.3"  # the herdr the table was checked against
# name -> (panel_bg, active_row_bg, text, subtext0, overlay1, overlay0)
THEMES: dict[str, dict] = {
    "catppuccin": _t("#181825", "#1e1e2e", "#cdd6f4", "#a6adc8", "#7f849c", "#6c7086"),
    "catppuccin-latte": _t("#eff1f5", "#e6e9ef", "#4c4f69", "#6c6f85", "#8c8fa1", "#9ca0b0"),
    "tokyo-night": _t("#1a1b26", "#232636", "#c0caf5", "#a9b1d6", "#697196", "#565f89"),
    "tokyo-night-day": _t("#e1e2e7", "#d2d3da", "#3760bf", "#6172b0", "#68709a", "#8990b3"),
    "dracula": _t("#282a36", "#373c52", "#f8f8f2", "#d2d2dc", "#828cb4", "#6272a4"),
    "nord": _t("#2e3440", "#434c5e", "#eceff4", "#d8dee9", "#646e82", "#4c566a"),
    "gruvbox": _t("#282828", "#323130", "#ebdbb2", "#d5c4a1", "#a89984", "#928374"),
    "gruvbox-light": _t("#fbf1c7", "#f2e5bc", "#3c3836", "#504945", "#7c6f64", "#928374"),
    "one-dark": _t("#282c34", "#313640", "#abb2bf", "#969ca8", "#737a87", "#5c6370"),
    "one-light": _t("#fafafa", "#d8dbe2", "#383a42", "#686b77", "#686b77", "#a0a1a7"),
    "solarized": _t("#002b36", "#164b57", "#93a1a1", "#839496", "#657b83", "#586e75"),
    "solarized-light": _t("#fdf6e3", "#eee8d5", "#657b83", "#839496", "#586e75", "#93a1a1"),
    "kanagawa": _t("#1f1f28", "#363646", "#dcd7ba", "#c8c3aa", "#87867d", "#727169"),
    "kanagawa-lotus": _t("#f2ecbc", "#d5cea3", "#545464", "#43436c", "#8a8980", "#a09cac"),
    "rose-pine": _t("#191724", "#26233a", "#e0def4", "#c8c5dc", "#908caa", "#6e6a86"),
    "rose-pine-dawn": _t("#faf4ed", "#e3d9cf", "#464261", "#797593", "#797593", "#9893a5"),
    "vesper": _t("#1a1a1a", "#101010", "#ffffff", "#a0a0a0", "#7e7e7e", "#5c5c5c"),
    # "terminal" follows the host terminal's ANSI palette, which panorama cannot read: none
}
# Every token is measured (the review of #19, the last nine at cell level). catppuccin's panel_bg
# is herdr's #181825 (mantle), not the palette's base (the operator's choice: follow herdr).
UNVERIFIED: set[tuple[str, str]] = set()
DEFAULT = "catppuccin"
RESET = frozenset({"reset", "default", "none", "transparent"})
NAMED = {"black": "#000000", "white": "#ffffff", "red": "#ff0000", "green": "#00ff00", "blue": "#0000ff",
         "yellow": "#ffff00", "cyan": "#00ffff", "magenta": "#ff00ff", "gray": "#808080", "grey": "#808080"}
_HEX = re.compile(r"^#([0-9a-fA-F]{6})$")
_RGB = re.compile(r"^rgb\(\s*(\d{1,3})\s*,\s*(\d{1,3})\s*,\s*(\d{1,3})\s*\)$")

_cache: dict = {"path": None, "mtime": None, "tokens": None, "cfg": {}}


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
    _load()
    return _cache["tokens"]


def herdr_config() -> dict:
    """herdr's config.toml as read (re-read when it changes); {} when missing or unreadable."""
    _load()
    return _cache["cfg"]


def _load() -> None:
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
        _cache.update(path=path, mtime=mtime, tokens=resolve(cfg), cfg=cfg)


DEFAULT_TINT = 0.15  # the operator's preference


def _settings() -> dict:
    from . import config
    return config.load()


def focus_tint(cfg: dict | None = None) -> float:
    """``focus_tint`` in panorama's config.toml (0..1, default 0.15): how far the focused panel's
    background goes from its unfocused one toward the theme's active_row_bg (0: no change,
    1: the full active_row_bg, which the operator found far too light)."""
    cfg = _settings() if cfg is None else cfg
    try:
        v = float(cfg.get("focus_tint", DEFAULT_TINT))
    except (TypeError, ValueError):
        v = DEFAULT_TINT
    return max(0.0, min(1.0, v))


def own_colors(cfg: dict | None = None) -> tuple[str | None, str | None]:
    """Panorama's own overrides in its config.toml, ``panel_bg`` (the unfocused background) and
    ``focus_bg`` (the focused one), hex; unset (or not a colour): follow herdr's theme."""
    cfg = _settings() if cfg is None else cfg
    out = []
    for key in ("panel_bg", "focus_bg"):
        c = color(cfg.get(key)) if key in cfg else None
        out.append(c or None)
    return out[0], out[1]


def _mix(a: str, b: str, k: float) -> str:
    """``a`` moved ``k`` of the way to ``b`` (hex colours, rounded half up like JS Math.round)."""
    pa = [int(a[i:i + 2], 16) for i in (1, 3, 5)]
    pb = [int(b[i:i + 2], 16) for i in (1, 3, 5)]
    return "#" + "".join(f"{int(x + (y - x) * k + 0.5):02x}" for x, y in zip(pa, pb, strict=True))


FIXED = ("#14161b", "#101215")  # (focused, not focused): the operator's default, tuned live


def mode(cfg: dict | None = None) -> str:
    """``panel_colors``: "fixed" (default: FIXED, panorama's own text colours) or "herdr" (follow
    herdr's theme, design turn 10, with ``focus_tint``)."""
    cfg = _settings() if cfg is None else cfg
    return "herdr" if str(cfg.get("panel_colors", "fixed")).lower() == "herdr" else "fixed"


def panel_colors(tokens: dict | None, focused: bool, fg: str, dim: str, faint: str,
                 fallback: tuple[str, str], cfg: dict | None = None) -> tuple[str, dict[str, str]]:
    """The docked panel's background and its text colours (panorama's FG, DIM, FAINT mapped),
    focused or not. The one place these colours are chosen.

    ``panel_colors = "fixed"`` (the default, the operator's choice): FIXED, #14161b focused and
    #101215 not; panorama's own text colours.

    ``panel_colors = "herdr"`` (design turn 10): herdr's theme.
    - not focused: the theme's panel_bg; text, dim -> overlay1, faint -> overlay0 (one step dimmer)
    - focused: the unfocused background moved ``focus_tint`` (default 0.15) of the way toward
      the theme's active_row_bg; text, dim -> subtext0, faint -> overlay1

    In both, panorama's own ``panel_bg`` / ``focus_bg`` (hex) replace the two backgrounds.
    ``cfg``: panorama's config (default: read it). In the herdr mode a token that is unset or
    "reset" falls back to panorama's own colour; ``fallback`` is (active, not focused)
    background for the "terminal" theme and a reset background."""
    cfg = _settings() if cfg is None else cfg
    own_panel, own_focus = own_colors(cfg)
    if mode(cfg) == "fixed":
        return (own_focus or FIXED[0]) if focused else (own_panel or FIXED[1]), {}
    k = focus_tint(cfg)
    base = own_panel or (tokens or {}).get("panel_bg") or fallback[1]
    if not focused:
        bg = base
    elif own_focus:
        bg = own_focus
    else:
        bg = _mix(base, (tokens or {}).get("active_row_bg") or fallback[0], k)
    if tokens is None:
        return bg, {}
    pairs = {fg: tokens["text"], dim: tokens["subtext0"] if focused else tokens["overlay1"],
             faint: tokens["overlay1"] if focused else tokens["overlay0"]}
    return bg, {k_: v for k_, v in pairs.items() if v}


HOVER_STEP = 0.04  # the hover row: this far from the panel's background toward white


def hover_bg(bg: str) -> str:
    """The docked list's hover / cursor row: a step lighter than the panel's current background,
    so it shows on both (#14161b -> #1d1f24, #101215 -> #1a1b1e; the operator: the
    fixed hover #171a1f vanished into a focused background of nearly that colour)."""
    return _mix(bg, "#ffffff", HOVER_STEP)


def panel_look(tokens: dict | None, focused: bool, fg: str, dim: str, faint: str,
               fallback: tuple[str, str], cfg: dict | None = None) -> tuple[str, dict[str, str], str]:
    """``panel_colors`` plus the hover row's background: what the view hands the renderer."""
    bg, fgs = panel_colors(tokens, focused, fg, dim, faint, fallback, cfg)
    return bg, fgs, hover_bg(bg)
