"""Open herdr-inbox on a specific question group (its deep-link API, herdr-inbox >= 0.6.0).

``inbox open --question Qn | --pane P`` (also ``--group``, ``--session``) writes a one-shot
focus request next to the inbox database and opens the inbox popup; the popup starts on that
group. It is run from the inbox's own plugin root, found with ``herdr plugin list --json``:

    uv run --quiet --frozen --no-dev --project <inbox plugin_root> inbox open --pane <pane>

with the inbox's own ``HERDR_PLUGIN_ID`` / ``HERDR_PLUGIN_ROOT`` (panorama's environment would
make it open panorama's pane). Support is detected by version (>= 0.6.0) or by ``inbox open
--help`` mentioning ``--pane``; without it, or if anything fails, the plain ``open`` action runs
as before. ``--no-hold`` (herdr-inbox >= 0.6.1: a normal popup, not one held open) is passed
only when ``inbox open --help`` lists it: panorama opens the inbox only on an explicit ``a`` or
click. Never raises.
"""

from __future__ import annotations

import json
import os
import re
import subprocess
import time

from . import herdr

INBOX_ID = "leavemyyard.herdr-inbox"
DEEP_LINK_VERSION = (0, 6, 0)
RECHECK_SECONDS = 60.0  # "not installed" is looked up again after this; a found inbox is kept
# panorama's own invocation context must not reach the inbox's command
STRIP = ("HERDR_PLUGIN_CONTEXT_JSON", "HERDR_PANE_ID", "HERDR_TAB_ID", "HERDR_WORKSPACE_ID",
         "HERDR_PLUGIN_CONFIG_DIR", "HERDR_PLUGIN_STATE_DIR", "HERDR_PLUGIN_ENTRYPOINT_ID")
# a failed deep link falls back to the plain popup only if it certainly failed before any popup
_BEFORE_POPUP = re.compile(r"could not open the inbox popup|unrecognized arguments|invalid choice|"
                           r"Unable to find lockfile|No such file or directory|Failed to spawn", re.I)
_cache: dict = {}


def plugin_info() -> dict | None:
    """The inbox plugin's ``{plugin_root, version}`` from herdr, or None (not installed/off)."""
    stale = _cache.get("info") is None and time.monotonic() - _cache.get("info_at", 0.0) >= RECHECK_SECONDS
    if "info" not in _cache or stale:
        out, _ = herdr.run_raw("plugin", "list", "--json")
        info = None
        try:
            data = json.loads(out or "{}")
            res = data.get("result", data) if isinstance(data, dict) else data
            plugins = res.get("plugins", []) if isinstance(res, dict) else res
            for p in plugins or []:
                if isinstance(p, dict) and p.get("plugin_id") == INBOX_ID and p.get("enabled", True):
                    info = {"plugin_root": p.get("plugin_root"), "version": str(p.get("version") or "")}
        except (ValueError, AttributeError):
            info = None
        _cache["info"], _cache["info_at"] = info, time.monotonic()
    return _cache["info"]


def _version(text: str) -> tuple[int, ...]:
    m = re.match(r"(\d+)\.(\d+)(?:\.(\d+))?", text or "")
    return tuple(int(g or 0) for g in m.groups()) if m else (0,)


def _env(root: str) -> dict:
    env = {k: v for k, v in os.environ.items() if not k.startswith(("UV_", "VIRTUAL_ENV")) and k not in STRIP}
    env.update(HERDR_PLUGIN_ID=INBOX_ID, HERDR_PLUGIN_ROOT=root)
    return env


def _inbox(root: str, *args: str, timeout: float = 20.0) -> tuple[int, str]:
    argv = ["uv", "run", "--quiet", "--frozen", "--no-dev", "--project", root, "inbox", *(herdr.clean(a) for a in args)]
    try:
        p = subprocess.run(argv, capture_output=True, text=True, timeout=timeout, env=_env(root), cwd=root)
    except subprocess.TimeoutExpired:
        return 124, "timeout"  # it may have opened the popup: no fallback
    except (OSError, subprocess.SubprocessError) as exc:
        return 127, f"Failed to spawn: {exc}"
    return p.returncode, (p.stdout or "") + (p.stderr or "")


def _help(info: dict) -> str | None:
    """``inbox open --help`` of this inbox (root and version), read once; None if it failed."""
    key = ("help", info["plugin_root"], info.get("version"))
    if key not in _cache:
        rc, out = _inbox(info["plugin_root"], "open", "--help")
        _cache[key] = out if rc == 0 else None
    return _cache[key]


def supports_deep_link(info: dict | None = None) -> bool:
    info = info or plugin_info()
    if not info or not info.get("plugin_root"):
        return False
    key = ("deep", info["plugin_root"], info.get("version"))
    if key not in _cache:
        if _version(info.get("version", "")) >= DEEP_LINK_VERSION:
            _cache[key] = True
        else:
            _cache[key] = "--pane" in (_help(info) or "")
    return _cache[key]


def supports_no_hold(info: dict) -> bool:
    """``inbox open --help`` lists ``--no-hold`` (probed once per inbox root and version; a
    failed probe means no)."""
    return "--no-hold" in (_help(info) or "")


def open_inbox(question: int | None = None, pane: str | None = None) -> str | None:
    """Open the inbox on ``question``'s group, else on ``pane``'s oldest open group, else plain."""
    info = plugin_info()
    if (question is not None or pane) and supports_deep_link(info):
        args = ["open", "--question", f"Q{question}"] if question is not None else ["open", "--pane", pane]
        if supports_no_hold(info):
            args.append("--no-hold")
        rc, out = _inbox(info["plugin_root"], *args)
        if rc == 0 or not _BEFORE_POPUP.search(out or ""):
            return None if rc == 0 else f"inbox open failed (exit {rc}); no second popup"
    return herdr.invoke_action(herdr.INBOX_ACTION)
