"""The key bound to panorama's open action, as herdr's config.toml has it (issue #13), for the
docked panel's footer: ``[[keys.command]]`` with ``type = "plugin_action"`` and
``command = "<plugin id>.open"``. Re-read when that file changes (theme.herdr_config).

``current()`` is ``(full, short)``: the footer shows the full key when it fits, else the short
form: a prefix binding reads ``pfx+`` then the rest, modifiers are letters joined with ``-``
(``ctrl+alt+p`` -> ``c-a-p``, ``prefix+space`` -> ``pfx+spc``, today's default)."""

from __future__ import annotations

import os

from . import herdr, theme

DEFAULT = ("prefix+space", "pfx+spc")  # the suggested binding (README), shown when none is read
WORDS = {"space": "spc", "ctrl": "c", "alt": "a", "shift": "s", "super": "d", "cmd": "d", "enter": "ret",
         "escape": "esc", "backspace": "bs"}


def short(key: str) -> str:
    parts = [p for p in key.strip().lower().split("+") if p]
    head = ""
    if parts and parts[0] == "prefix":
        head, parts = "pfx+", parts[1:]
    return head + "-".join(WORDS.get(p, p) for p in parts)


def binding(cfg: dict) -> str | None:
    action = f"{os.environ.get('HERDR_PLUGIN_ID', herdr.PLUGIN_ID)}.open"
    keys = cfg.get("keys") if isinstance(cfg, dict) else None
    for entry in (keys.get("command") if isinstance(keys, dict) else None) or []:
        if isinstance(entry, dict) and entry.get("type") == "plugin_action" and entry.get("command") == action:
            key = entry.get("key")
            if isinstance(key, str) and key.strip():
                return herdr.clean(key.strip(), 40)
    return None


def current() -> tuple[str, str]:
    key = binding(theme.herdr_config())
    return (key, short(key)) if key else DEFAULT
