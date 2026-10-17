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

class NoReply(str):
    """A socket request's error when it was sent but no reply came (a timeout, a dropped
    connection): herdr may have done it all the same. Any other error is a refusal."""


def socket_call(method: str, params: dict, timeout: float = TIMEOUT) -> tuple[dict | None, str | None]:
    """One raw socket API request (newline-delimited JSON) for methods the CLI does not wrap,
    such as ``tab.move``. Never raises; the error is a ``NoReply`` when the request went out
    unanswered."""
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
    sent = False
    try:
        with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as s:
            s.settimeout(timeout)
            s.connect(path)
            s.sendall(req.encode())
            sent = True
            buf = b""
            while b"\n" not in buf:
                chunk = s.recv(65536)
                if not chunk:
                    break
                buf += chunk
        if not buf.strip():
            return None, NoReply("closed without a reply")  # not a success: nobody said it was done
        reply = json.loads(buf.split(b"\n", 1)[0])
    except OSError as exc:
        return None, NoReply(str(exc) or "timed out") if sent else str(exc)
    except ValueError as exc:
        return None, NoReply(f"garbled reply: {exc}")  # only a sent request gets this far
    if not isinstance(reply, dict) or "error" in reply:
        return None, str((reply or {}).get("error") if isinstance(reply, dict) else reply)
    return reply.get("result") or {}, None


NEW_AGENT_POPUP = (84, 26)  # the popup's outer size in cells (the form is 76 wide)


def open_new_agent(cwd: str) -> str | None:
    """The + New agent form in its own herdr popup (the plugin's ``new-agent`` entrypoint), with
    ``cwd`` as its default directory. None, or the error (a ``NoReply`` when herdr may have
    opened it without answering)."""
    plugin_id = os.environ.get("HERDR_PLUGIN_ID", PLUGIN_ID)
    params = {"plugin_id": plugin_id, "entrypoint": "new-agent", "placement": "popup", "focus": True,
              "width": NEW_AGENT_POPUP[0], "height": NEW_AGENT_POPUP[1],
              "env": {"PANORAMA_NEW_AGENT_CWD": cwd}}
    root = os.environ.get("HERDR_PLUGIN_ROOT")
    if root:
        params["cwd"] = root
    return socket_call("plugin.pane.open", params)[1]


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


# The per-tab panel's command, as in the manifest's ``panel`` pane (a test keeps them equal):
# ``layout.apply`` panes are not plugin panes, so a + New agent tab's panel is started with it.
# The venv's own entry point (the plugin's build ran `uv sync`): `uv run` would keep a ~31 MB
# uv parent alive beside every panel. `uv run` only when there is no venv.
PANEL_COMMAND = ["sh", "-c", 'R="${HERDR_PLUGIN_ROOT:-.}"; [ -x "$R/.venv/bin/panorama" ] && exec "$R/.venv/bin/panorama" panel; '
                 'exec uv run --quiet --frozen --no-dev --project "$R" panorama panel']
PANEL_LABEL = "Panorama panel"  # the manifest's title for the panel entrypoint; herdr labels the pane with it
PLUGIN_ENV = ("HERDR_PLUGIN_ID", "HERDR_PLUGIN_ROOT", "HERDR_PLUGIN_STATE_DIR", "HERDR_PLUGIN_CONFIG_DIR")


def open_panel(target_pane: str) -> tuple[dict | None, str | None]:
    """A per-tab panel (the plugin's ``panel`` entrypoint) split right of ``target_pane``,
    without focus: the operator stays where they are. Returns the PaneInfo."""
    plugin_id = os.environ.get("HERDR_PLUGIN_ID", PLUGIN_ID)
    args = ["plugin", "pane", "open", "--plugin", plugin_id, "--entrypoint", "panel", "--placement", "split",
            "--direction", "right", "--target-pane", target_pane, "--no-focus"]
    root = os.environ.get("HERDR_PLUGIN_ROOT")
    if root:
        args += ["--cwd", root]
    res, err = call(*args)
    if res is None:
        return None, err
    pane = (res.get("plugin_pane") or {}).get("pane") or {}
    return (pane, None) if pane.get("pane_id") else (None, "no pane in the reply")


def close_plugin_pane(pane_id: str) -> str | None:
    return call("plugin", "pane", "close", pane_id)[1]


def close_pane(pane_id: str) -> str | None:
    """``pane close``: any pane, plugin or not. Used only for a per-tab panel that herdr's
    session restore brought back as a plain shell (panels.Manager proves it is one first);
    herdr no longer counts such a pane as a plugin pane, so ``plugin pane close`` refuses it."""
    return call("pane", "close", pane_id)[1]


def _etime_seconds(text: str) -> float | None:
    """``ps -o etime`` (``[[dd-]hh:]mm:ss``) in seconds, or None."""
    days, _, rest = text.strip().rpartition("-")
    try:
        parts = [int(x) for x in rest.split(":")]
        d = int(days) if days else 0
    except ValueError:
        return None
    if not 2 <= len(parts) <= 3:
        return None
    h, m, s = ([0] + parts)[-3:]
    return float(((d * 24 + h) * 60 + m) * 60 + s)


def process_age(pid: int) -> float | None:
    """How long the process has run, in seconds (``ps -o etime=``, Linux and macOS), or None."""
    try:
        proc = subprocess.run(["ps", "-o", "etime=", "-p", str(int(pid))], capture_output=True, text=True,
                              timeout=TIMEOUT)
    except (OSError, subprocess.SubprocessError, ValueError):
        return None
    return _etime_seconds(proc.stdout) if proc.returncode == 0 else None


def bare_shell(pane_id: str) -> tuple[bool | None, float | None]:
    """Whether the pane's foreground is nothing but the pane's own shell (one foreground
    process, the pane's ``shell_pid``, not panorama): what herdr's session restore leaves in a
    pane whose command it does not start again. With the age in seconds of the pane's shell
    (None when unknown). (None, None) when herdr cannot say. Read-only ``pane process-info``."""
    res, _ = call("pane", "process-info", "--pane", pane_id)
    info = (res or {}).get("process_info")
    if not isinstance(info, dict):
        return None, None
    procs = info.get("foreground_processes")
    shell = info.get("shell_pid")
    if not isinstance(procs, list) or not procs or not isinstance(shell, int):
        return None, None  # mid-exec or mid-restore: unknown
    procs = [p for p in procs if isinstance(p, dict)]
    if any(os.path.basename(str(a)) == "panorama" for p in procs for a in p.get("argv") or []):
        return False, process_age(shell)  # a live panel: its sh -c execs panorama as the shell's pid
    bare = len(procs) == 1 and procs[0].get("pid") == shell
    return bare, process_age(shell)  # not bare: something runs in it (a command, or the rc's own)


def apply_panel_layout(label: str, cwd: str | None, ratio: float) -> tuple[dict | None, str | None]:
    """A new, unfocused tab with a per-tab panel on the left and a plain pane on the right (for
    + New agent: no panel ever has to be swapped in there). Returns the layout: its ``tab_id``
    and ``root.first`` / ``root.second`` pane ids."""
    root = os.environ.get("HERDR_PLUGIN_ROOT")
    env = {k: os.environ[k] for k in PLUGIN_ENV if os.environ.get(k)}
    first = {"type": "pane", "command": PANEL_COMMAND, "env": env, "label": PANEL_LABEL}
    if root:
        first["cwd"] = root
    second = {"type": "pane"}
    if cwd:
        second["cwd"] = cwd
    res, err = socket_call("layout.apply", {"tab_label": clean(label, 200), "focus": False, "root": {
        "type": "split", "direction": "right", "ratio": ratio, "first": first, "second": second}})
    if res is None:
        return None, err
    lay = res.get("layout") or {}
    return (lay, None) if lay.get("tab_id") else (None, "no tab in the reply")


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


def runs_panorama(pane_id: str, command: str = "tui", pid: int | None = None) -> bool | None:
    """Whether the pane's foreground process is panorama's ``command`` (``tui`` the view,
    ``panel`` a per-tab panel): some foreground argv holds ``panorama`` (any path to it, as uv
    and the venv script both run it) followed by ``command``; with ``pid``, that very process
    must be this pid (a panel proving the pane it claims is its own). Read-only
    ``pane process-info``. None when herdr cannot say."""
    res, _ = call("pane", "process-info", "--pane", pane_id)
    if res is None:
        return None
    procs = (res.get("process_info") or {}).get("foreground_processes")
    if not isinstance(procs, list) or not procs:
        return None  # mid-exec or mid-restore: unknown, not "something else"
    for proc in procs:
        argv = proc.get("argv") if isinstance(proc, dict) else None
        if isinstance(argv, list) and (pid is None or proc.get("pid") == pid):
            words = [str(a) for a in argv]
            if any(os.path.basename(w) == "panorama" and nxt == command for w, nxt in zip(words, words[1:], strict=False)):
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
