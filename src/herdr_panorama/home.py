"""Panorama as the operator's home surface: a dedicated tab, first in the tab bar. (With the
experimental cockpit, per-tab panels sit left of each orchestrator; the home view stays here.)

- **Single instance, found by content.** The home pane carries the pane label
  ``◆ panorama``, runs no agent, and herdr's ``pane process-info`` shows ``panorama tui``
  in its foreground; every such pane in the snapshot is a candidate, wherever it sits.
  Look-alikes (a shell with the label, another plugin's pane) are never touched. One
  is used (the calling instance's, else the remembered one); a second verified panorama
  is left alone, never adopted or closed. An older home pane (the only agent-less
  pane of a tab labelled ``◆ panorama``) is adopted and labelled. The record is per herdr
  session (``home-<hash of the socket path>.json`` in the plugin state dir). Opening runs
  under a file lock and re-checks after taking it, so the startup hook and the key can
  never open two.
- **Opening.** ``herdr plugin pane open --placement tab`` (``PANORAMA_HOME=1`` in its
  environment), then ``pane rename`` and ``tab rename`` to ``◆ panorama`` and the socket
  API's ``tab.move`` to position 0 (the CLI has no ``tab move``).
- **Config.** ``home_tab = true|false`` in the plugin's config.toml (default true). Off:
  the open action shows the popup, as before.
- **New agent.** ``tab create --label <name> --cwd <dir> --focus`` -> ``agent start <name>
  --kind claude --pane <the tab's pane>`` (waits for readiness) -> ``agent prompt`` once
  with the first prompt, if any. This is the only place panorama types into a pane, and
  only on the operator's submit; every argument passes ``herdr.clean``.
"""

from __future__ import annotations

import hashlib
import json
import os
import re
from contextlib import contextmanager
from dataclasses import dataclass
from pathlib import Path

from . import config, herdr

LOCK_SECONDS = 10.0  # the longest any caller waits for the home lock
QUIT_LOCK_SECONDS = 0.5  # quit (UI thread) waits at most this long
HOME_LABEL = "◆ panorama"  # the home tab's label, and the home pane's own label
INBOX_MARK = re.compile(r"^(?:❓\d+\s+)+")  # herdr-inbox's open-question prefix on a tab label
NAME_RULE = re.compile(r"^[a-z][a-z0-9_-]{0,31}$")  # herdr's agent-name rule


class LockBusy(Exception):
    """The home lock stayed held by someone else for the whole wait."""


def state_dir() -> Path:
    env = os.environ.get("HERDR_PLUGIN_STATE_DIR")
    if env:
        return Path(env)
    state = os.environ.get("XDG_STATE_HOME") or os.path.join(os.path.expanduser("~"), ".local", "state")
    return Path(state) / "herdr-panorama"


def session_key() -> str:
    """One herdr session (server socket) -> one record: two ``herdr --session`` servers
    never read or overwrite each other's home."""
    sock = os.environ.get("HERDR_SOCKET_PATH") or "default"
    return hashlib.sha1(sock.encode()).hexdigest()[:12]


def enabled() -> bool:
    value = config.load().get("home_tab", True)
    return value is not False and str(value).lower() not in ("false", "0", "no", "off")


def is_home() -> bool:
    """This process runs in the home pane (not a popup or split)."""
    return os.environ.get("PANORAMA_HOME") == "1"


# -- finding panorama's pane ------------------------------------------------------------

@dataclass
class Home:
    pane_id: str
    tab_id: str


def _record() -> Path:
    return state_dir() / f"home-{session_key()}.json"


def _load() -> dict:
    try:
        data = json.loads(_record().read_text())
        return data if isinstance(data, dict) else {}
    except (OSError, ValueError):
        return {}


def _save(home: Home) -> None:
    d = state_dir()
    d.mkdir(parents=True, exist_ok=True)
    tmp = d / f"home-{session_key()}.json.tmp"
    tmp.write_text(json.dumps({"pane_id": home.pane_id, "tab_id": home.tab_id}))
    os.replace(tmp, _record())


def is_home_label(label) -> bool:
    """``◆ panorama``, also with herdr-inbox's ``❓N`` prefix."""
    return INBOX_MARK.sub("", str(label or "")) == HOME_LABEL


def _tab_label(snapshot: dict, tab_id) -> str:
    tab = next((t for t in snapshot.get("tabs") or [] if isinstance(t, dict) and t.get("tab_id") == tab_id), {})
    return str(tab.get("label") or "")


def _lookalikes(snapshot: dict) -> list[dict]:
    """Panes that *look* like panorama's home pane, wherever they are (its own tab, or
    docked next to an agent by an older version's moving cockpit):

    - a pane labelled ``◆ panorama`` (the pane label, set when it is opened) that runs
      no agent;
    - an older home pane: the only agent-less pane of a tab labelled ``◆ panorama``.

    Looks are not proof: a shell left after panorama quit, a pane whose agent just exited,
    another plugin's pane or a label restored without its process all look like this."""
    panes = [p for p in snapshot.get("panes") or [] if isinstance(p, dict) and p.get("pane_id")]
    out = [p for p in panes if p.get("label") == HOME_LABEL and not p.get("agent")]
    for t in snapshot.get("tabs") or []:
        if isinstance(t, dict) and is_home_label(t.get("label")):
            mine = [p for p in panes if p.get("tab_id") == t.get("tab_id") and not p.get("agent")]
            if len(mine) == 1 and mine[0] not in out:
                out.append(mine[0])
    return out


def scan(snapshot: dict) -> tuple[list[dict], bool]:
    """(the panes verified to run panorama's view, whether some look-alike could not be
    checked). Only a verified pane is ever adopted, renamed, moved or closed."""
    verified, unknown = [], False
    for p in _lookalikes(snapshot):
        runs = herdr.runs_panorama(p["pane_id"])
        if runs:
            verified.append(p)
        elif runs is None:
            unknown = True
    return verified, unknown


def neutral_label(snapshot: dict, panes: list[dict]) -> str:
    """The label a tab gets when panorama is no longer in it: the remaining agent's herdr
    name, else its kind (``claude``), else ``tab``."""
    names = {a.get("pane_id"): a.get("name") for a in snapshot.get("agents") or [] if isinstance(a, dict)}
    agents = [p for p in panes if p.get("agent")]
    return (next((names.get(p["pane_id"]) or p.get("name") for p in agents
                  if names.get(p["pane_id"]) or p.get("name")), None)
            or (str(agents[0]["agent"]) if agents else "tab"))


def _in_tab(snapshot: dict, tab_id) -> list[dict]:
    return [p for p in snapshot.get("panes") or [] if isinstance(p, dict) and p.get("tab_id") == tab_id]


def release_tab(snapshot: dict, tab_id, leaving: str) -> str | None:
    """Panorama's pane ``leaving`` is about to leave (dock) or close (quit): if its tab is a
    home tab that still holds other panes, it loses the ``◆ panorama`` label first, so
    nothing is ever left behind under panorama's label. Error or None."""
    if not is_home_label(_tab_label(snapshot, tab_id)):
        return None
    others = [p for p in _in_tab(snapshot, tab_id) if p.get("pane_id") != leaving]
    if not others:
        return None  # panorama alone: the tab closes with its pane
    return herdr.rename_tab(tab_id, neutral_label(snapshot, others))


def sweep(snapshot: dict) -> None:
    """Every ``◆ panorama`` tab that holds no panorama is renamed (after a crash, or a quit
    that could not rename it). A tab keeps the label while any agent-less pane in it is
    panorama, or cannot be checked (it might be panorama, mid-start or mid-restore)."""
    for t in snapshot.get("tabs") or []:
        if not (isinstance(t, dict) and is_home_label(t.get("label"))):
            continue
        panes = _in_tab(snapshot, t.get("tab_id"))
        if any(herdr.runs_panorama(p["pane_id"]) is not False for p in panes if not p.get("agent")):
            continue
        herdr.rename_tab(t["tab_id"], neutral_label(snapshot, panes))


def leave(pane_id: str | None, timeout: float = QUIT_LOCK_SECONDS) -> str | None:
    """On quit: release the tab this panorama pane is in (see ``release_tab``). Runs on the
    UI thread, so it never waits long for the lock: if a dock or follow holds it, the relabel
    is skipped and the sweep in the next ``ensure`` covers it."""
    if not pane_id:
        return None
    try:
        with _lock(timeout):
            snapshot = herdr.snapshot()
            if snapshot is None:
                return "herdr did not answer"
            tab = next((p.get("tab_id") for p in snapshot.get("panes") or []
                        if isinstance(p, dict) and p.get("pane_id") == pane_id), None)
            return release_tab(snapshot, tab, pane_id) if tab else None
    except LockBusy:
        return "busy: relabel skipped (the next start sweeps it)"


def candidates(snapshot: dict) -> list[dict]:
    return scan(snapshot)[0]


def find(snapshot: dict, found: list[dict] | None = None, prefer: str | None = None) -> Home | None:
    """The live home pane (verified): ``prefer`` (the calling instance's own pane) if it is
    a candidate, else the remembered one, else the first."""
    found = candidates(snapshot) if found is None else found
    if not found:
        return None
    known = _load().get("pane_id")
    p = (next((c for c in found if c["pane_id"] == prefer), None)
         or next((c for c in found if c["pane_id"] == known), found[0]))
    home = Home(p["pane_id"], p.get("tab_id") or "")
    if p["pane_id"] != known:
        _save(home)
    return home


@contextmanager
def _lock(timeout: float | None = None):
    """The per-session home lock. Never waits forever: after ``timeout`` (default
    LOCK_SECONDS) it raises LockBusy, so a stuck holder can never hang the key, the startup
    hook or a quit."""
    import fcntl
    import time
    d = state_dir()
    d.mkdir(parents=True, exist_ok=True)
    fd = os.open(d / f"home-{session_key()}.lock", os.O_WRONLY | os.O_CREAT, 0o644)
    try:
        end = time.monotonic() + (LOCK_SECONDS if timeout is None else timeout)
        while True:
            try:
                fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
                break
            except BlockingIOError:
                if time.monotonic() >= end:
                    raise LockBusy("the panorama home lock is busy") from None
                time.sleep(0.02)
        try:
            yield
        finally:
            fcntl.flock(fd, fcntl.LOCK_UN)
    finally:
        os.close(fd)


def tab_label(snapshot: dict, tab_id) -> str:
    return _tab_label(snapshot, tab_id)


def ensure(focus: bool) -> tuple[Home | None, str | None]:
    """Panorama's home pane, opened if missing (never a second one); focused when asked.

    Reconciles by content first: one verified panorama is kept (a second one is left alone,
    never adopted or closed), and a pane docked next to an agent (in a tab that is not a home
    tab; the moving cockpit of an older version left it there) goes back to a home tab of its
    own. A home tab the operator split is left as it is."""
    from . import cockpit
    try:
        with _lock():
            home, err, final = _ensure_locked(focus, cockpit)
    except LockBusy:
        return None, "another panorama operation holds the home lock; try again"
    if final or home is None:
        return home, err
    if focus:  # outside the lock, as before
        err = herdr.focus_tab(home.tab_id)
        if err:
            return home, err
    return home, None


def _ensure_locked(focus: bool, cockpit):
    """``ensure`` under the home lock: find or open, reconcile. (home, error, final): final
    means nothing is left to do (a fresh home tab was opened with the focus asked for)."""
    snapshot = herdr.snapshot()
    if snapshot is None:
        # unknown is not "missing": opening now could make a second home tab and overwrite
        # the record (a slow herdr at startup); the next start or key press tries again
        return None, "herdr did not answer; not opening the home tab", True
    found, unknown = scan(snapshot)
    sweep(snapshot)  # no ◆ panorama tab without panorama in it (issue #9)
    home = find(snapshot, found)
    if home is None and unknown:
        # a look-alike herdr could not check might be the live panorama: neither adopt it
        # nor open a second one; the key falls back to the popup and tries again later
        return None, "could not verify panorama's pane; not opening the home tab", True
    if home is None:
        pane, err = herdr.open_tab_pane(focus)
        if pane is None:
            return None, err, True
        home = Home(pane["pane_id"], pane["tab_id"])
        _save(home)
        herdr.rename_pane(home.pane_id, HOME_LABEL)
        herdr.rename_tab(home.tab_id, HOME_LABEL)
        herdr.move_tab(home.tab_id, 0)  # best effort: an older herdr may lack tab.move
        return home, None, True
    if herdr.pane_label(snapshot, home.pane_id) != HOME_LABEL:
        herdr.rename_pane(home.pane_id, HOME_LABEL)  # an older home pane: label it once
    if not is_home_label(_tab_label(snapshot, home.tab_id)):
        # the home panorama stays home: a pane docked next to an agent (the moving cockpit of
        # an older version) goes back to its own tab
        moved = cockpit.undock(home)
        if moved is not None:
            home = moved
    return home, None, False


# -- new agent ----------------------------------------------------------------------------

def slugify(name: str) -> str:
    """A herdr agent name (``[a-z][a-z0-9_-]{0,31}``) from free text; '' if nothing is left."""
    s = re.sub(r"[^a-z0-9_-]+", "-", name.strip().lower()).strip("-_")
    s = re.sub(r"-{2,}", "-", s)
    if s and not s[0].isalpha():
        s = "a-" + s
    return s[:32].rstrip("-_")


def new_agent(name: str, cwd: str | None, prompt: str | None) -> tuple[str | None, str | None]:
    """Create the tab (unfocused), start Claude Code in its pane, send the first prompt once to
    that pane, then focus the tab. On a failed start the tab is closed again. (name, error)."""
    slug = slugify(name)
    if not NAME_RULE.match(slug):
        return None, "the name needs at least one letter"
    if cwd and not os.path.isdir(cwd):
        return None, f"no such directory: {cwd}"
    if herdr.agent_exists(slug):
        return None, f"an agent named {slug} already exists"
    from . import cockpit
    panel = None
    if cockpit.per_tab():
        # per-tab panels: the tab is built with its panel on the left from the first frame
        # (layout.apply), so no panel ever has to be swapped in there
        lay, err = herdr.apply_panel_layout(slug, cwd or None, 0.3)
        if lay is None:
            return None, f"tab create failed: {err}"
        tab = {"tab_id": lay["tab_id"]}
        root = lay.get("root") or {}
        panel = (root.get("first") or {}).get("pane_id")
        pane = (root.get("second") or {}).get("pane_id")
    else:
        tab, err = herdr.create_tab(slug, cwd or None, focus=False)
        if tab is None or not tab.get("tab_id"):
            return None, f"tab create failed: {err}"
        pane = herdr.tab_root_pane(tab["tab_id"])
    if pane is None:
        herdr.close_tab(tab["tab_id"])
        return None, "the new tab has no pane"
    if panel:
        cockpit.fit(panel, tab["tab_id"])
    err = herdr.start_agent(slug, pane)
    if err:
        herdr.close_tab(tab["tab_id"])  # no orphan tab left behind
        return None, f"agent start failed: {err}"
    text = herdr.clean(prompt or "").strip()
    perr = herdr.prompt_agent(pane, text) if text else None  # the pane we just created, by id
    herdr.focus_tab(tab["tab_id"])
    if panel:
        herdr.focus_pane(pane)  # the agent, not the panel layout.apply left focused
    if perr:
        return slug, f"started, but the first prompt failed: {perr}"
    return slug, None
