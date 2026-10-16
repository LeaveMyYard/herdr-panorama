"""Home tab (single instance, first position, focus) and the + New agent flow, against a
stateful fake herdr and a fake socket for ``tab.move``."""

import json
import os
import shutil
import socket
import tempfile
import sys
import threading
from pathlib import Path

import pytest

from herdr_panorama import home
from herdr_panorama.cli import main as cli

FAKE = r'''#!{python}
import json, os, sys
args = sys.argv[1:]
log, state_path = os.environ["FAKE_HERDR_LOG"], os.environ["FAKE_HERDR_STATE"]
with open(log, "a") as fh:
    fh.write(json.dumps(args) + "\n")
st = json.load(open(state_path)) if os.path.exists(state_path) else {{"tabs": [], "panes": [], "n": 0}}
cmd = " ".join(args[:2]) if args[:1] != ["plugin"] else " ".join(args[:3])
fail = os.environ.get("FAKE_HERDR_FAIL", "").split(",")
if cmd in fail:
    sys.stderr.write("boom"); sys.exit(1)
def new_tab(label):
    st["n"] += 1
    t, p = f"w1:t{{st['n']}}", f"w1:p{{st['n']}}"
    st["tabs"].append({{"tab_id": t, "label": label, "workspace_id": "w1"}})
    st["panes"].append({{"pane_id": p, "tab_id": t, "workspace_id": "w1"}})
    return t, p
result = {{"type": "ok"}}
if cmd == "api snapshot":
    focus = next((p for p in st["panes"] if p["pane_id"] == st.get("focus")), {{}})
    result = {{"snapshot": {{"agents": [], "tabs": st["tabs"], "panes": st["panes"], "focused_pane_id": focus.get("pane_id"),
               "focused_tab_id": focus.get("tab_id"), "focused_workspace_id": focus.get("workspace_id")}},
               "type": "session_snapshot"}}
elif cmd == "plugin pane open":
    t, p = new_tab("Panorama")
    st["panes"][-1]["argv"] = ["uv", "run", "--quiet", "--project", "/plugins/panorama", "panorama", "tui"]
    result = {{"plugin_pane": {{"plugin_id": "leavemyyard.herdr-panorama", "entrypoint": "panorama",
                               "pane": {{"pane_id": p, "tab_id": t}}}}, "type": "plugin_pane_opened"}}
elif cmd == "tab rename":
    for t in st["tabs"]:
        if t["tab_id"] == args[2]:
            t["label"] = args[3]
elif cmd == "tab create":
    label = args[args.index("--label") + 1]
    t, p = new_tab(label)
    result = {{"tab": {{"tab_id": t, "label": label}}, "type": "tab_created"}}
elif cmd == "pane list":
    result = {{"panes": st["panes"], "type": "pane_list"}}
elif cmd == "agent start":
    for p in st["panes"]:
        if p["pane_id"] == args[args.index("--pane") + 1]:
            p["agent"] = "claude"
    st.setdefault("names", []).append(args[2])
elif cmd == "agent get":
    if args[2] not in st.get("names", []):
        sys.stderr.write("agent_not_found"); sys.exit(1)
    result = {{"agent": {{"pane_id": "x"}}}}
elif cmd == "agent prompt":
    # herdr 0.9.3: `agent prompt <TARGET> <TEXT> [OPTIONS]`: the word after the target is the text
    # whatever it looks like (`--`, `--wait`); every later word must be a known option.
    target, text, rest = args[2], args[3], args[4:]
    while rest:
        opt = rest.pop(0)
        if opt == "--wait":
            continue
        if opt in ("--until", "--timeout") and rest:
            rest.pop(0); continue
        sys.stderr.write(f"unknown option: {{opt}}\n"); sys.exit(2)
    st.setdefault("prompted", []).append([target, text])
elif cmd == "pane process-info":
    pane = next((p for p in st["panes"] if p["pane_id"] == args[args.index("--pane") + 1]), None)
    if pane is None:
        sys.stderr.write("pane_not_found"); sys.exit(1)
    argv = pane["argv"] if "argv" in pane else (["claude"] if pane.get("agent") else ["zsh"])
    result = {{"type": "pane_process_info", "process_info": {{"pane_id": pane["pane_id"], "foreground_processes": [
        {{"argv": argv, "name": argv[0]}}] if argv else []}}}}
elif cmd == "pane rename":
    for p in st["panes"]:
        if p["pane_id"] == args[2]:
            p["label"] = " ".join(args[3:])
elif cmd == "plugin pane close":
    gone = next((p for p in st["panes"] if p["pane_id"] == args[3]), None)
    st["panes"] = [p for p in st["panes"] if p["pane_id"] != args[3]]
    if gone and not any(p["tab_id"] == gone["tab_id"] for p in st["panes"]):
        st["tabs"] = [t for t in st["tabs"] if t["tab_id"] != gone["tab_id"]]
elif cmd == "pane swap":
    a = args[args.index("--source-pane") + 1]
    b = args[args.index("--target-pane") + 1]
    pa = next(p for p in st["panes"] if p["pane_id"] == a)
    pb = next(p for p in st["panes"] if p["pane_id"] == b)
    if pa["tab_id"] != pb["tab_id"]:
        result = {{"type": "pane_swap", "swap": {{"changed": False, "reason": "cross_tab"}}}}
    else:
        i, j = st["panes"].index(pa), st["panes"].index(pb)
        st["panes"][i], st["panes"][j] = pb, pa
        # a swap focuses its source pane (it has no --no-focus), here only inside the tab the
        # client shows. This models the operator's observed client revert (the client stayed on
        # the old tab), not herdr 0.9.3's server state right after the swap, which reports the
        # source focused in its tab (the review of #20, with an isolated attached client)
        if next((p["tab_id"] for p in st["panes"] if p["pane_id"] == st.get("focus")), None) == pa["tab_id"]:
            st["focus"] = a
        result = {{"type": "pane_swap", "swap": {{"changed": True, "focused_pane_id": a}}}}
elif cmd == "pane move":
    # herdr 0.9.3: within a workspace the pane keeps its id; a tab left empty is closed
    variant = "pane move --new-tab" if "--new-tab" in args else "pane move --tab"
    if variant in fail:
        sys.stderr.write("boom"); sys.exit(1)
    pane = next((p for p in st["panes"] if p["pane_id"] == args[2]), None)
    if pane is None:
        sys.stderr.write("pane_not_found"); sys.exit(1)
    old, created = pane["tab_id"], None
    src_ws = pane.get("workspace_id") or "w1"
    if "--new-tab" in args:
        ws = args[args.index("--workspace") + 1] if "--workspace" in args else src_ws
        st["n"] += 1
        created = {{"tab_id": f"{{ws}}:t{{st['n']}}", "label": args[args.index("--label") + 1], "workspace_id": ws}}
        st["tabs"].append(created)
        pane["tab_id"] = created["tab_id"]
    else:
        pane["tab_id"] = args[args.index("--tab") + 1]
        ws = next((t.get("workspace_id") for t in st["tabs"] if t["tab_id"] == pane["tab_id"]), src_ws)
    if ws != src_ws:  # herdr 0.9.3: a pane moved into another workspace gets a new id
        st["n"] += 1
        pane["pane_id"], pane["workspace_id"] = f"{{ws}}:p{{st['n']}}", ws
    if "--new-tab" not in args:
        st["panes"].remove(pane)
        st["panes"].append(pane)  # split right: after the panes already there
    closed = None
    if st.get("focus") == args[2] and "--no-focus" in args:
        # herdr 0.9.3: moving the focused pane with --no-focus leaves the client on its old tab,
        # on the pane left there (when the tab survives)
        rest = [p["pane_id"] for p in st["panes"] if p["tab_id"] == old]
        if rest:
            st["focus"] = rest[0]
    if not any(p["tab_id"] == old for p in st["panes"]):
        st["tabs"] = [t for t in st["tabs"] if t["tab_id"] != old]
        closed = old
    result = {{"type": "pane_move", "move_result": {{"pane": {{"pane_id": pane["pane_id"], "tab_id": pane["tab_id"]}},
              "previous_pane_id": args[2], "previous_tab_id": old, "closed_tab_id": closed, "created_tab": created,
              "target_layout": {{"area": {{"width": int(os.environ.get("FAKE_HERDR_AREA", "200"))}}}}}}}}
elif cmd == "tab focus":
    # the client shows that tab, on its focused pane (here: the one already focused there, else its first)
    tab = args[2]
    in_tab = [p["pane_id"] for p in st["panes"] if p["tab_id"] == tab]
    if in_tab and st.get("focus") not in in_tab:
        st["focus"] = in_tab[0]
elif cmd == "tab close":
    st["tabs"] = [t for t in st["tabs"] if t["tab_id"] != args[2]]
    st["panes"] = [p for p in st["panes"] if p["tab_id"] != args[2]]
json.dump(st, open(state_path, "w"))
print(json.dumps({{"id": "cli", "result": result}}))
'''


class Herdr:
    def __init__(self, tmp: Path, monkeypatch):
        self.bin, self.log, self.state = tmp / "herdr", tmp / "calls.jsonl", tmp / "state.json"
        self.bin.write_text(FAKE.format(python=sys.executable))
        self.bin.chmod(0o755)
        self.log.touch()
        monkeypatch.setenv("HERDR_BIN_PATH", str(self.bin))
        monkeypatch.setenv("FAKE_HERDR_LOG", str(self.log))
        monkeypatch.setenv("FAKE_HERDR_STATE", str(self.state))
        monkeypatch.setenv("HERDR_PLUGIN_STATE_DIR", str(tmp / "plugin-state"))
        self.moves: list[dict] = []
        self.area = 200  # the tab width pane.layout reports
        # macOS caps Unix socket paths at 104 bytes and pytest's tmp paths are longer there
        self.sock_dir = tempfile.mkdtemp(prefix="hp", dir="/tmp")
        sock = os.path.join(self.sock_dir, "s")
        self.server = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        self.server.bind(sock)
        self.server.listen(8)
        monkeypatch.setenv("HERDR_SOCKET_PATH", sock)
        threading.Thread(target=self._serve, daemon=True).start()

    def _serve(self):
        while True:
            try:
                conn, _ = self.server.accept()
            except OSError:
                return
            with conn:
                req = json.loads(conn.makefile().readline())
                self.moves.append(req)
                result = {"type": "tab_moved"}
                p = req.get("params") or {}
                if req["method"] == "tab.move" and self.state.exists():
                    st = json.loads(self.state.read_text())
                    tab = next((t for t in st["tabs"] if t["tab_id"] == p["tab_id"]), None)
                    if tab:
                        st["tabs"].remove(tab)
                        st["tabs"].insert(p["insert_index"], tab)
                        self.state.write_text(json.dumps(st))
                elif req["method"] in ("pane.layout", "layout.export"):
                    # the tab's panes side by side, in order: a chain of right splits
                    st = json.loads(self.state.read_text())
                    tab = p.get("tab_id") or next((q["tab_id"] for q in st["panes"] if q["pane_id"] == p.get("pane_id")), None)
                    ids = [q["pane_id"] for q in st["panes"] if q["tab_id"] == tab]
                    w = self.area // max(1, len(ids))
                    if req["method"] == "pane.layout":
                        result = {"type": "pane_layout", "layout": {"area": {"width": self.area}, "panes": [
                            {"pane_id": q, "rect": {"x": i * w, "width": w}} for i, q in enumerate(ids)]}}
                    else:
                        node = {"type": "pane", "pane_id": ids[-1]} if ids else None
                        for q in reversed(ids[:-1]):
                            node = {"type": "split", "direction": "right", "ratio": 0.5,
                                    "first": {"type": "pane", "pane_id": q}, "second": node}
                        result = {"type": "layout_export", "layout": {"root": node}}
                else:
                    if req["method"] == "pane.focus" and self.state.exists():
                        st = json.loads(self.state.read_text())
                        st["focus"] = p.get("pane_id")
                        self.state.write_text(json.dumps(st))
                    result = {"type": "ok"}
                conn.sendall((json.dumps({"id": req["id"], "result": result}) + "\n").encode())

    def calls(self, *prefix):
        out = [json.loads(x) for x in self.log.read_text().splitlines() if x]
        return [c for c in out if c[:len(prefix)] == list(prefix)]

    def prompted(self):
        """What herdr accepted as prompt text, per pane, after parsing argv like the real CLI."""
        return json.loads(self.state.read_text()).get("prompted", []) if self.state.exists() else []

    def tabs(self):
        return json.loads(self.state.read_text())["tabs"] if self.state.exists() else []


@pytest.fixture
def herdr(tmp_path, monkeypatch):
    h = Herdr(tmp_path, monkeypatch)
    yield h
    h.server.close()
    shutil.rmtree(h.sock_dir, ignore_errors=True)


def test_home_opens_once_renamed_first_and_focused_on_demand(herdr):
    h, err = home.ensure(focus=False)
    assert err is None and h.tab_id == "w1:t1"
    opened = herdr.calls("plugin", "pane", "open")
    assert len(opened) == 1 and "--placement" in opened[0] and "tab" in opened[0] and "--no-focus" in opened[0]
    assert "PANORAMA_HOME=1" in opened[0]
    assert herdr.tabs() == [{"tab_id": "w1:t1", "label": home.HOME_LABEL, "workspace_id": "w1"}]
    assert [m["method"] for m in herdr.moves] == ["tab.move"]
    assert herdr.moves[0]["params"] == {"tab_id": "w1:t1", "insert_index": 0}
    assert home.ensure(focus=False)[0] == h  # never a second one
    home.ensure(focus=True)
    assert len(herdr.calls("plugin", "pane", "open")) == 1
    assert herdr.calls("tab", "focus") == [["tab", "focus", "w1:t1"]]


def test_concurrent_openers_make_one_home(herdr):
    results = []
    threads = [threading.Thread(target=lambda: results.append(home.ensure(focus=False))) for _ in range(6)]
    [t.start() for t in threads]
    [t.join() for t in threads]
    assert len(herdr.calls("plugin", "pane", "open")) == 1
    assert {r[0].tab_id for r in results} == {"w1:t1"}


def test_lost_record_adopts_the_labelled_tab_and_a_gone_home_is_reopened(herdr):
    home.ensure(focus=False)
    home._record().unlink()
    assert home.ensure(focus=False)[0].tab_id == "w1:t1"  # adopted, not reopened
    assert len(herdr.calls("plugin", "pane", "open")) == 1
    st = json.loads(herdr.state.read_text())
    st["tabs"], st["panes"] = [], []  # the operator closed the home tab
    herdr.state.write_text(json.dumps(st))
    assert home.ensure(focus=False)[0].tab_id == "w1:t2"
    assert len(herdr.calls("plugin", "pane", "open")) == 2


def test_open_action_focuses_home_and_popup_stays_available(herdr):
    assert cli(["open"]) == 0
    assert "--focus" in herdr.calls("plugin", "pane", "open")[0]  # opened with focus
    assert cli(["open"]) == 0  # the same key from anywhere: back home
    assert len(herdr.calls("plugin", "pane", "open")) == 1
    assert herdr.calls("tab", "focus") == [["tab", "focus", "w1:t1"]]
    assert cli(["open", "--popup"]) == 0
    popup = herdr.calls("plugin", "pane", "open")[-1]
    assert "--placement" not in popup  # the manifest's popup placement


def test_home_tab_off_means_popup_and_no_startup_tab(herdr, tmp_path, monkeypatch):
    cfg = tmp_path / "plugin-config"
    cfg.mkdir(exist_ok=True)
    (cfg / "config.toml").write_text("home_tab = false\n")
    assert cli(["home", "--startup"]) == 0
    assert herdr.calls("plugin", "pane", "open") == []
    assert cli(["open"]) == 0
    assert "--placement" not in herdr.calls("plugin", "pane", "open")[0]


def test_startup_does_not_take_focus(herdr):
    assert cli(["home", "--startup"]) == 0
    assert "--no-focus" in herdr.calls("plugin", "pane", "open")[0] and herdr.calls("tab", "focus") == []


@pytest.mark.parametrize("name,slug", [("Billing API!", "billing-api"), ("  9 lives ", "a-9-lives"),
                                       ("x" * 40, "x" * 32), ("Ünïcödé", "n-c-d"), ("!!!", ""),
                                       ("fix_auth--flow", "fix_auth-flow")])
def test_slugify(name, slug):
    assert home.slugify(name) == slug
    assert not slug or home.NAME_RULE.match(slug)


def test_new_agent_flow_sanitises_and_prompts_once(herdr, tmp_path):
    work = tmp_path / "billing"
    work.mkdir()
    name, err = home.new_agent("Billing API!", str(work), "fix\x1b[2J the bug\nnow")
    assert (name, err) == ("billing-api", None)
    assert herdr.calls("tab", "create") == [["tab", "create", "--label", "billing-api", "--no-focus", "--cwd",
                                             str(work)]]
    assert herdr.calls("tab", "focus") == [["tab", "focus", "w1:t1"]]  # focused once the agent runs
    assert herdr.calls("agent", "start") == [["agent", "start", "billing-api", "--kind", "claude", "--pane",
                                              "w1:p1", "--timeout", "60000"]]
    assert herdr.prompted() == [["w1:p1", "fix[2J the bug now"]]


def test_new_agent_without_prompt_and_failures(herdr, monkeypatch):
    assert home.new_agent("docs", None, "  ")[0] == "docs"
    assert herdr.calls("agent", "prompt") == []
    assert home.new_agent("!!!", None, "x") == (None, "the name needs at least one letter")
    monkeypatch.setenv("FAKE_HERDR_FAIL", "agent start")
    name, err = home.new_agent("search", None, "go")
    assert name is None and err.startswith("agent start failed")
    assert len(herdr.calls("agent", "prompt")) == 0  # never typed when the agent did not start



# -- review of #5 -----------------------------------------------------------------------------

def test_r5_h1_a_failed_snapshot_never_opens_a_second_home(herdr, monkeypatch):
    home.ensure(focus=False)
    record = home._record().read_text()
    monkeypatch.setenv("FAKE_HERDR_FAIL", "api snapshot")
    h, err = home.ensure(focus=True)
    assert h is None and err  # unknown: do not open, try again later
    assert len(herdr.calls("plugin", "pane", "open")) == 1
    assert home._record().read_text() == record
    assert cli(["open"]) == 0  # the key still works: the popup instead
    assert "--placement" not in herdr.calls("plugin", "pane", "open")[-1]


def test_r5_m5_a_record_is_trusted_only_if_it_still_looks_like_home(herdr):
    home.ensure(focus=False)
    st = json.loads(herdr.state.read_text())
    st["tabs"][0]["label"] = "work"  # pane id reused by something else: another label, an agent
    st["panes"][0]["agent"] = "claude"
    herdr.state.write_text(json.dumps(st))
    assert home.ensure(focus=False)[0].tab_id == "w1:t2"  # a new home, not the impostor
    assert len(herdr.calls("plugin", "pane", "open")) == 2


def test_r5_m1_prompt_goes_to_the_new_pane_and_cannot_be_a_flag(herdr):
    name, err = home.new_agent("docs", None, "--wait please")
    assert err is None
    assert herdr.prompted() == [["w1:p1", "--wait please"]]


def test_r5_m1_an_existing_agent_name_is_refused(herdr):
    home.new_agent("docs", None, None)
    name, err = home.new_agent("Docs", None, "hi")
    assert name is None and "already" in err
    assert len(herdr.calls("tab", "create")) == 1 and herdr.calls("agent", "prompt") == []


def test_r5_m2_a_failed_start_closes_its_tab(herdr, monkeypatch):
    monkeypatch.setenv("FAKE_HERDR_FAIL", "agent start")
    name, err = home.new_agent("search", None, "go")
    assert name is None and err.startswith("agent start failed")
    assert herdr.calls("tab", "close") == [["tab", "close", "w1:t1"]] and herdr.tabs() == []
    assert herdr.calls("tab", "focus") == []  # it never took focus


def test_r5_l3_a_missing_directory_is_refused(herdr, tmp_path):
    name, err = home.new_agent("docs", str(tmp_path / "nope"), None)
    assert name is None and "directory" in err and herdr.calls("tab", "create") == []


def test_r5b_h1_the_prompt_is_the_word_after_the_target_as_herdr_parses_it(herdr):
    # herdr 0.9.3 reads `agent prompt <TARGET> <TEXT> [OPTIONS]`; a `--` there became the text and
    # the real prompt an "unknown option", so every first prompt failed.
    for i, text in enumerate(["hello", "--", "--wait", "--timeout 5", "-x"]):
        name, err = home.new_agent(f"a{i}", None, text)
        assert err is None, err
    assert [t for _, t in herdr.prompted()] == ["hello", "--", "--wait", "--timeout 5", "-x"]
    assert all(len(c) == 4 for c in herdr.calls("agent", "prompt"))  # target and text, nothing else
