"""User settings: ``config.toml`` in the herdr plugin config dir (the herdr-inbox pattern).

Location: ``$HERDR_PLUGIN_CONFIG_DIR`` (set by herdr for plugin commands), else
``herdr plugin config-dir leavemyyard.herdr-panorama``. Re-read when the file
changes. Keys:

- ``done_window``: how long finished threads stay visible. Seconds, or a
  duration string such as ``"90m"``, ``"1h"``, ``"2h30m"``. Default 1 hour.
  The ``PANORAMA_DONE_WINDOW`` environment variable (seconds) overrides it.
- ``home_tab``: panorama in its own first tab (default true).
- ``machines``: herdr's saved SSH machines to show, "all" (default), "none" or a list of
  labels (machines.py).
- ``cockpit`` / ``cockpit_width``: experimental, panorama docked left of the selected agent (cockpit.py);
  ``dock_keep_focus`` (default true): Enter in the docked list keeps the keyboard on panorama.
- ``highlight_unseen``: pulse what failed, asked or finished since the operator looked (unseen.py),
  default true.
- ``animation_fps``: how often colours move (pulses, blinks), 2 to 30, default 20. Lower costs less
  CPU and looks steppier; spinners and sweeps keep their speed. Read when the view starts.
"""

from __future__ import annotations

import logging
import os
import re
import tomllib
from pathlib import Path

from . import herdr

log = logging.getLogger("herdr_panorama.config")

DEFAULT_DONE_WINDOW = 3600.0
_DUR = re.compile(r"^\s*(?:(\d+)\s*h)?\s*(?:(\d+)\s*m)?\s*(?:(\d+)\s*s)?\s*$")
_cache: dict = {"dir": None, "mtime": None, "data": {}}


def config_dir() -> str | None:
    if os.environ.get("HERDR_PLUGIN_CONFIG_DIR"):
        return os.environ["HERDR_PLUGIN_CONFIG_DIR"]
    if _cache["dir"] is None:
        _cache["dir"] = herdr.plugin_config_dir(herdr.PLUGIN_ID) or ""
    return _cache["dir"] or None


def load() -> dict:
    d = config_dir()
    if not d:
        return {}
    path = Path(d) / "config.toml"
    try:
        mtime = path.stat().st_mtime
    except OSError:
        return {}
    if mtime != _cache["mtime"]:
        try:
            with open(path, "rb") as fh:
                _cache["data"] = tomllib.load(fh)
        except (OSError, tomllib.TOMLDecodeError) as exc:
            log.warning("ignoring %s: %s", path, exc)
            _cache["data"] = {}
        _cache["mtime"] = mtime
    return _cache["data"]


def parse_duration(value) -> float | None:
    if isinstance(value, bool):
        return None
    if isinstance(value, (int, float)):
        return float(value) if value >= 0 else None
    if isinstance(value, str):
        if value.strip().isdigit():
            return float(value.strip())
        m = _DUR.match(value.lower())
        if m and any(m.groups()):
            h, mi, s = (int(g or 0) for g in m.groups())
            return float(h * 3600 + mi * 60 + s)
    return None


def done_window() -> float:
    env = os.environ.get("PANORAMA_DONE_WINDOW")
    if env:
        val = parse_duration(env)
        if val is not None:
            return val
    val = parse_duration(load().get("done_window"))
    return DEFAULT_DONE_WINDOW if val is None else val


DEFAULT_ANIMATION_FPS = 20
ANIMATION_FPS_RANGE = (2, 30)


def animation_fps() -> float:
    """``animation_fps`` clamped to 2..30 (a typo or a non-number: the default)."""
    value = load().get("animation_fps", DEFAULT_ANIMATION_FPS)
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        log.warning("animation_fps: not a number (%r), using %s", value, DEFAULT_ANIMATION_FPS)
        return float(DEFAULT_ANIMATION_FPS)
    lo, hi = ANIMATION_FPS_RANGE
    return float(min(max(value, lo), hi))
