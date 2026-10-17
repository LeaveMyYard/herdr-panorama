"""The key bound to panorama's open action, as herdr's config.toml has it (issue #13), for the
docked panel's footer: ``[[keys.command]]`` with ``type = "plugin_action"`` and
``command = "<plugin id>.open"``. Re-read when that file changes (theme.herdr_config).

``current()`` is ``(full, short)``. The footer (render._open_key_hints, the operator's rule,
Q157) shows the key whole, dropping the ``n new`` hint if that makes it fit, and the short form
only when even that is not enough: a prefix binding reads ``pfx+`` then the rest, modifiers are
letters joined with ``-`` (``ctrl+alt+p`` -> ``c-a-p``). The suggested ``prefix+space``, and no
binding at all, show the design's own ``pfx+spc``."""

from __future__ import annotations

import os

from . import herdr, theme

# The suggested binding (README) is the design's footer text, `pfx+spc`, whole: shown for it, and
# when no binding is read (the design keeps every hint; the full "prefix+space" would drop n new)
DEFAULT = ("pfx+spc", "pfx+spc")
SUGGESTED = "prefix+space"
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
    return (key, short(key)) if key and key.lower() != SUGGESTED else DEFAULT
