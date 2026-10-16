"""The only place that talks to the herdr CLI.

Every call has a short timeout and never raises: failures are logged and the
caller gets ``None`` (or an error string). The swarm view only ever *reads*
herdr state, plus two explicit operator actions: focusing a pane and invoking
the herdr-inbox popup. Nothing is typed into any pane.
"""

from __future__ import annotations

import json
import logging
import os
import re
import subprocess

log = logging.getLogger("herdr_panorama.herdr")

TIMEOUT = 3.0
PLUGIN_ID = "leavemyyard.herdr-panorama"
INBOX_ACTION = "leavemyyard.herdr-inbox.open"

MAX_ARG = 4000
_SPACE = re.compile(r"[\t\n\r\v\f  \x85]")
_CONTROL = re.compile(r"[\x00-\x1f\x7f-\x9f]")


def clean(text: str, limit: int = MAX_ARG) -> str:
    """Make text safe to show in a terminal or pass to herdr.

    Line and tab breaks become spaces; every other C0/C1 control (ESC, BEL, CSI, ...)
    and DEL is dropped, so no escape or key sequence survives. Capped at ``limit``.
    """
    text = _CONTROL.sub("", _SPACE.sub(" ", str(text)))
    return text if len(text) <= limit else text[: limit - 1] + "…"


def _bin() -> str:
    return os.environ.get("HERDR_BIN_PATH") or "herdr"


def trace(op: str, started: float, **extra) -> None:
    """``PANORAMA_TRACE=<file>``: one JSON line per herdr call or traced step, with its
    duration in ms. Off by default; never raises."""
    path = os.environ.get("PANORAMA_TRACE")
    if not path:
        return
    import time as _t
    try:
        with open(path, "a") as fh:
            fh.write(json.dumps({"at": round(_t.time(), 3), "op": op,
                                 "ms": round((_t.perf_counter() - started) * 1000, 1), **extra}) + "\n")
    except OSError:
        pass


def run_raw(*args: str, timeout: float | None = None) -> tuple[str | None, str | None]:
    """Run ``herdr <args>``; return (stdout, None) or (None, error)."""
    import time as _t
    t0 = _t.perf_counter()
    try:
        return _run_raw(*args, timeout=timeout)
    finally:
        trace("cli " + " ".join(args[:2]), t0)


def _run_raw(*args: str, timeout: float | None = None) -> tuple[str | None, str | None]:
    argv = [_bin(), *(clean(a) for a in args)]  # every argument, one choke point
    try:
        proc = subprocess.run(argv, capture_output=True, text=True, timeout=timeout or TIMEOUT)
    except (OSError, subprocess.SubprocessError) as exc:
        log.warning("herdr %s failed: %s", args[:2], exc)
        return None, str(exc)
    if proc.returncode != 0:
        err = (proc.stderr or proc.stdout).strip() or f"exit {proc.returncode}"
        log.warning("herdr %s failed: %s", args[:2], err)
        return None, err
    return proc.stdout, None


def call(*args: str, timeout: float | None = None) -> tuple[dict | None, str | None]:
    """Run ``herdr <args>``; return (``.result`` dict, None) or (None, error)."""
    out, err = run_raw(*args, timeout=timeout)
    if out is None:
        return None, err
    try:
        data = json.loads(out) if out.strip() else {}
    except json.JSONDecodeError:
        return {}, None
    return (data.get("result") or {}) if isinstance(data, dict) else {}, None


def snapshot() -> dict | None:
    """The live session snapshot (agents, panes, tabs, workspaces), or None."""
    res, _ = call("api", "snapshot")
    if res is None:
        return None
    snap = res.get("snapshot")
    return snap if isinstance(snap, dict) else None


def focus_agent(pane_id: str) -> str | None:
    """Focus the agent in ``pane_id``. Returns an error or None."""
    _, err = call("agent", "focus", pane_id)
    return err


def invoke_action(action_id: str) -> str | None:
    _, err = call("plugin", "action", "invoke", action_id)
    return err


def plugin_config_dir(plugin_id: str) -> str | None:
    out, _ = run_raw("plugin", "config-dir", plugin_id)
    return (out.strip() or None) if out else None


def invocation_pane() -> str | None:
    """The pane that was focused when herdr invoked this plugin command, if herdr says."""
    try:
        ctx = json.loads(os.environ.get("HERDR_PLUGIN_CONTEXT_JSON") or "{}")
    except json.JSONDecodeError:
        ctx = {}
    return (ctx.get("focused_pane_id") if isinstance(ctx, dict) else None) or None


def open_view(split: bool = False) -> str | None:
    """Open the plugin's ``panorama`` pane: popup by default, a right split with ``split``."""
    plugin_id = os.environ.get("HERDR_PLUGIN_ID", PLUGIN_ID)
    args = ["plugin", "pane", "open", "--plugin", plugin_id, "--entrypoint", "panorama"]
    root = os.environ.get("HERDR_PLUGIN_ROOT")
    if root:
        args += ["--cwd", root]
    if split:
        args += ["--placement", "split", "--direction", "right", "--focus"]
        target = invocation_pane() or os.environ.get("HERDR_PANE_ID")
        if target:
            args += ["--target-pane", target]
    _, err = call(*args)
    return err


# -- home tab and new agents ---------------------------------------------------------

def socket_call(method: str, params: dict, timeout: float = TIMEOUT) -> tuple[dict | None, str | None]:
    """One raw socket API request (newline-delimited JSON) for methods the CLI does not wrap,
    such as ``tab.move``. Never raises."""
    import time as _t
    t0 = _t.perf_counter()
    try:
        return _socket_call(method, params, timeout)
    finally:
        trace("socket " + method, t0)


def _socket_call(method: str, params: dict, timeout: float = TIMEOUT) -> tuple[dict | None, str | None]:
    import socket
    path = os.environ.get("HERDR_SOCKET_PATH")
    if not path:
        return None, "HERDR_SOCKET_PATH is not set"
    req = json.dumps({"id": "panorama:1", "method": method, "params": params}) + "\n"
    try:
        with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as s:
            s.settimeout(timeout)
            s.connect(path)
            s.sendall(req.encode())
            buf = b""
            while b"\n" not in buf:
                chunk = s.recv(65536)
                if not chunk:
                    break
                buf += chunk
        reply = json.loads(buf.split(b"\n", 1)[0] or b"{}")
    except (OSError, ValueError) as exc:
        return None, str(exc)
    if not isinstance(reply, dict) or "error" in reply:
        return None, str((reply or {}).get("error") if isinstance(reply, dict) else reply)
    return reply.get("result") or {}, None


def open_tab_pane(focus: bool) -> tuple[dict | None, str | None]:
    """Open the plugin's ``panorama`` entrypoint as its own tab; returns the PaneInfo."""
    plugin_id = os.environ.get("HERDR_PLUGIN_ID", PLUGIN_ID)
    args = ["plugin", "pane", "open", "--plugin", plugin_id, "--entrypoint", "panorama", "--placement", "tab",
            "--env", "PANORAMA_HOME=1", "--focus" if focus else "--no-focus"]
    root = os.environ.get("HERDR_PLUGIN_ROOT")
    if root:
        args += ["--cwd", root]
    res, err = call(*args)
    if res is None:
        return None, err
    pane = (res.get("plugin_pane") or {}).get("pane") or {}
    return (pane, None) if pane.get("pane_id") else (None, "no pane in the reply")


def rename_tab(tab_id: str, label: str) -> str | None:
    return call("tab", "rename", tab_id, label)[1]


def focus_tab(tab_id: str) -> str | None:
    return call("tab", "focus", tab_id)[1]


def move_tab(tab_id: str, index: int) -> str | None:
    return socket_call("tab.move", {"tab_id": tab_id, "insert_index": index})[1]


def create_tab(label: str, cwd: str | None, focus: bool = True) -> tuple[dict | None, str | None]:
    args = ["tab", "create", "--label", label, "--focus" if focus else "--no-focus"]
    if cwd:
        args += ["--cwd", cwd]
    res, err = call(*args)
    return ((res or {}).get("tab"), None) if res is not None else (None, err)


def tab_root_pane(tab_id: str) -> str | None:
    res, _ = call("pane", "list")
    return next((p["pane_id"] for p in (res or {}).get("panes", []) if p.get("tab_id") == tab_id and p.get("pane_id")),
                None)


def start_agent(name: str, pane_id: str, kind: str = "claude", timeout_ms: int = 60000) -> str | None:
    return call("agent", "start", name, "--kind", kind, "--pane", pane_id, "--timeout", str(timeout_ms),
                timeout=timeout_ms / 1000 + 5)[1]


def prompt_agent(target: str, text: str) -> str | None:
    """Type ``text`` into the agent in pane ``target`` as one prompt (sanitised by ``clean``
    like every argument). herdr reads ``agent prompt <TARGET> <TEXT> [OPTIONS]``: the word after
    the target is the text whatever it looks like (``--wait`` included), so no ``--`` goes in
    front of it; herdr 0.9.3 would take a ``--`` as the text and refuse the prompt as an option."""
    return call("agent", "prompt", target, text)[1]


def agent_exists(name: str) -> bool:
    """Whether herdr already resolves ``name`` to an agent (read-only ``agent get``)."""
    res, _ = call("agent", "get", name)
    return bool(res)


def close_tab(tab_id: str) -> str | None:
    return call("tab", "close", tab_id)[1]


# -- home pane and cockpit (experimental): only panorama's own pane ever moves ----------

def rename_pane(pane_id: str, label: str) -> str | None:
    return call("pane", "rename", pane_id, label)[1]


def pane_label(snapshot: dict, pane_id: str) -> str | None:
    p = next((p for p in snapshot.get("panes") or [] if isinstance(p, dict) and p.get("pane_id") == pane_id), {})
    return p.get("label")


def runs_panorama(pane_id: str) -> bool | None:
    """Whether the pane's foreground process is panorama's view: some foreground argv holds
    ``panorama`` (any path to it, as uv and the venv script both run it) followed by
    ``tui``. Read-only ``pane process-info``. None when herdr cannot say."""
    res, _ = call("pane", "process-info", "--pane", pane_id)
    if res is None:
        return None
    procs = (res.get("process_info") or {}).get("foreground_processes")
    if not isinstance(procs, list) or not procs:
        return None  # mid-exec or mid-restore: unknown, not "something else"
    for proc in procs:
        argv = proc.get("argv") if isinstance(proc, dict) else None
        if isinstance(argv, list):
            words = [str(a) for a in argv]
            if any(os.path.basename(w) == "panorama" and nxt == "tui" for w, nxt in zip(words, words[1:], strict=False)):
                return True
    return False


def claude_pid(pane_id: str) -> int | None:
    """The pid of the Claude Code process in the pane's foreground (read-only ``pane
    process-info``), or None."""
    from .hooks import is_claude
    res, _ = call("pane", "process-info", "--pane", pane_id)
    procs = ((res or {}).get("process_info") or {}).get("foreground_processes")
    for proc in procs if isinstance(procs, list) else []:
        if not isinstance(proc, dict) or not isinstance(proc.get("pid"), int):
            continue
        argv = [str(a) for a in proc.get("argv") or [] if isinstance(a, (str, int))]
        if is_claude(str(proc.get("name") or ""), argv):
            return proc["pid"]
    return None


def move_pane_to_tab(pane_id: str, tab_id: str, target_pane: str | None = None) -> tuple[dict | None, str | None]:
    """``pane move <pane> --tab <tab> --split right [--target-pane P] --no-focus``; returns
    ``move_result`` (its ``pane.pane_id`` is the pane's id from now on)."""
    args = ["pane", "move", pane_id, "--tab", tab_id, "--split", "right", "--no-focus"]
    if target_pane:
        args += ["--target-pane", target_pane]
    res, err = call(*args)
    return ((res or {}).get("move_result") or {}, None) if res is not None else (None, err)


def move_pane_new_tab(pane_id: str, workspace_id: str | None, label: str) -> tuple[dict | None, str | None]:
    """``pane move <pane> --new-tab [--workspace W] --label L --no-focus``; returns ``move_result``."""
    args = ["pane", "move", pane_id, "--new-tab", "--label", label, "--no-focus"]
    if workspace_id:
        args += ["--workspace", workspace_id]
    res, err = call(*args)
    return ((res or {}).get("move_result") or {}, None) if res is not None else (None, err)


def swap_panes(source: str, target: str) -> str | None:
    """Swap two panes of one tab (herdr refuses across tabs)."""
    res, err = call("pane", "swap", "--source-pane", source, "--target-pane", target)
    if res is None:
        return err
    swap = res.get("swap") or {}
    return None if swap.get("changed", True) else f"not swapped: {swap.get('reason')}"


def layout_export(tab_id: str) -> dict | None:
    res, _ = socket_call("layout.export", {"tab_id": tab_id})
    return ((res or {}).get("layout") or {}).get("root") if res else None


def set_split_ratio(tab_id: str, path: list[bool], ratio: float) -> str | None:
    """One split of the tab (``path``: False = first child, True = second, from the root);
    ``ratio`` is the first child's share."""
    return socket_call("layout.set_split_ratio", {"tab_id": tab_id, "path": path, "ratio": ratio})[1]


def pane_layout(pane_id: str) -> dict | None:
    res, _ = socket_call("pane.layout", {"pane_id": pane_id})
    return (res or {}).get("layout") if res else None


def focus_pane(pane_id: str) -> str | None:
    return socket_call("pane.focus", {"pane_id": pane_id})[1]


def zoom_pane(pane_id: str) -> str | None:
    """Toggle the pane's zoom (panorama full-screen in its tab, and back)."""
    return call("pane", "zoom", pane_id, "--toggle")[1]


def events(types: list[str], stop, timeout: float = 1.0):
    """herdr's event stream (``events.subscribe``): yields each event's ``data`` dict, for
    example ``{"type": "tab_focused", "tab_id", "workspace_id"}``, plus two markers of its own:
    ``{"type": "_connected"}`` when herdr acknowledges the subscription and
    ``{"type": "_disconnected"}`` when a live stream is lost. Reconnects (backing off) until
    ``stop()`` is true. Read-only. Never raises."""
    import socket
    import time as _time
    backoff = 0.5
    while not stop():
        path = os.environ.get("HERDR_SOCKET_PATH")
        if not path:
            return
        live = False
        try:
            with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as s:
                s.settimeout(timeout)
                s.connect(path)
                s.sendall((json.dumps({"id": "panorama:events", "method": "events.subscribe",
                                       "params": {"subscriptions": [{"type": t} for t in types]}}) + "\n").encode())
                buf = b""
                while not stop():
                    try:
                        chunk = s.recv(65536)
                    except socket.timeout:
                        continue
                    if not chunk:
                        break
                    buf += chunk
                    while b"\n" in buf:
                        line, buf = buf.split(b"\n", 1)
                        try:
                            msg = json.loads(line or b"{}")
                        except ValueError:
                            continue
                        if isinstance(msg, dict) and (msg.get("result") or msg.get("data")):
                            backoff = 0.5  # only a real ack or event proves the stream works
                            if not live:
                                live = True
                                yield {"type": "_connected"}
                        data = msg.get("data") if isinstance(msg, dict) else None
                        if isinstance(data, dict):
                            yield data
        except OSError as exc:
            log.warning("herdr events: %s", exc)
        if live:
            yield {"type": "_disconnected"}
        if not stop():
            _time.sleep(backoff)
            backoff = min(backoff * 2, 10.0)
