"""Deep-linking into herdr-inbox (>= 0.6.0 `inbox open --question/--pane`), the fallback to its
plain popup, and click targets that follow the arming and identity rules."""

import json
import sys

import pytest

from herdr_panorama import inbox_link
from herdr_panorama.tui import SwarmApp

from .demo import NOW, demo_swarm

HERDR = r'''#!{python}
import json, os, sys
args = sys.argv[1:]
open(os.environ["FAKE_LOG"], "a").write(json.dumps(["herdr", *args]) + "\n")
if args[:3] == ["plugin", "list", "--json"]:
    v = os.environ.get("FAKE_INBOX_VERSION")
    plugins = [] if not v else [{{"plugin_id": "leavemyyard.herdr-inbox", "enabled": True, "version": v,
                                  "plugin_root": os.environ["FAKE_INBOX_ROOT"], "source": {{"kind": "local"}}}}]
    print(json.dumps({{"id": "cli", "result": {{"plugins": plugins, "type": "plugin_list"}}}}))
else:
    print(json.dumps({{"id": "cli", "result": {{"type": "ok"}}}}))
'''

UV = r'''#!{python}
import json, os, sys
args = sys.argv[1:]
leaked = [k for k in ("HERDR_PLUGIN_CONTEXT_JSON", "HERDR_PANE_ID", "HERDR_PLUGIN_STATE_DIR") if k in os.environ]
open(os.environ["FAKE_LOG"], "a").write(json.dumps(["uv", *args, "LEAKED", leaked, "ENV",
                                                    os.environ.get("HERDR_PLUGIN_ID"),
                                                    os.environ.get("HERDR_PLUGIN_ROOT")]) + "\n")
tail = args[args.index("inbox") + 1:]
if tail == ["open", "--help"]:
    print(("usage: inbox open [--question Qn | --group G | --pane PANE_ID | --session SID]"
           if os.environ.get("FAKE_INBOX_HELP_PANE") == "1" else "usage: inbox open")
          + (" [--no-hold]" if os.environ.get("FAKE_INBOX_HELP_NOHOLD") == "1" else ""))
    sys.exit(0)
sys.stderr.write(os.environ.get("FAKE_INBOX_OUT", ""))
sys.exit(int(os.environ.get("FAKE_INBOX_RC", "0")))
'''


class Fakes:
    def __init__(self, tmp, monkeypatch):
        self.log = tmp / "log.jsonl"
        self.log.touch()
        bindir = tmp / "bin"
        bindir.mkdir()
        for name, body in (("herdr", HERDR), ("uv", UV)):
            (bindir / name).write_text(body.format(python=sys.executable))
            (bindir / name).chmod(0o755)
        self.root = tmp / "inbox-root"
        self.root.mkdir()
        monkeypatch.setenv("PATH", f"{bindir}:{__import__('os').environ['PATH']}")
        monkeypatch.setenv("HERDR_BIN_PATH", str(bindir / "herdr"))
        monkeypatch.setenv("FAKE_LOG", str(self.log))
        monkeypatch.setenv("FAKE_INBOX_ROOT", str(self.root))
        monkeypatch.setenv("HERDR_PLUGIN_ID", "leavemyyard.herdr-panorama")  # panorama's own env
        # a cold Python start of the fake can exceed herdr's 3 s on a busy CI runner (seen on macOS)
        monkeypatch.setattr(__import__("herdr_panorama.herdr").herdr, "TIMEOUT", 30.0)
        inbox_link._cache.clear()

    def calls(self, first):
        return [c[1:] for c in (json.loads(x) for x in self.log.read_text().splitlines()) if c[0] == first]

    def opens(self):
        return [c for c in self.calls("uv") if "open" in c and "--help" not in c]

    def plain(self):
        return [c for c in self.calls("herdr") if c[:3] == ["plugin", "action", "invoke"]]


@pytest.fixture
def fakes(tmp_path, monkeypatch):
    return Fakes(tmp_path, monkeypatch)


def test_060_deep_links_with_the_inbox_environment(fakes, monkeypatch):
    monkeypatch.setenv("FAKE_INBOX_VERSION", "0.6.0")
    assert inbox_link.open_inbox(question=2) is None
    (call,) = fakes.opens()
    assert call[:7] == ["run", "--quiet", "--frozen", "--no-dev", "--project", str(fakes.root), "inbox"]
    assert call[7:10] == ["open", "--question", "Q2"]
    assert call[-2:] == ["leavemyyard.herdr-inbox", str(fakes.root)]  # not panorama's plugin id
    assert fakes.plain() == []
    inbox_link.open_inbox(pane="w1:p3")
    assert fakes.opens()[-1][7:10] == ["open", "--pane", "w1:p3"]


def test_older_version_probes_help_for_pane(fakes, monkeypatch):
    monkeypatch.setenv("FAKE_INBOX_VERSION", "0.5.9")
    monkeypatch.setenv("FAKE_INBOX_HELP_PANE", "1")  # a dev build that already has deep links
    inbox_link.open_inbox(pane="w1:p1")
    assert fakes.opens() and fakes.plain() == []


def test_053_falls_back_to_the_plain_popup(fakes, monkeypatch):
    monkeypatch.setenv("FAKE_INBOX_VERSION", "0.5.3")
    inbox_link.open_inbox(question=2)
    assert fakes.opens() == []  # only the --help probe ran
    assert fakes.plain() == [["plugin", "action", "invoke", "leavemyyard.herdr-inbox.open"]]


def test_not_installed_or_failing_deep_link_falls_back(fakes, monkeypatch):
    inbox_link.open_inbox(question=2)  # no inbox plugin listed
    assert fakes.opens() == [] and len(fakes.plain()) == 1
    inbox_link._cache.clear()
    monkeypatch.setenv("FAKE_INBOX_VERSION", "0.6.1")
    monkeypatch.setenv("FAKE_INBOX_RC", "1")
    monkeypatch.setenv("FAKE_INBOX_OUT", "could not open the inbox popup: boom")  # failed before a popup
    inbox_link.open_inbox(question=2)
    assert len(fakes.opens()) == 1 and len(fakes.plain()) == 2
    assert inbox_link.open_inbox() is None and len(fakes.opens()) == 1  # no target: plain


# -- click targets ------------------------------------------------------------------------

async def started(home_mode=False):
    app = SwarmApp(source=demo_swarm, live_refresh=False, clock=lambda: NOW, focus_pane="w1:p1", home_mode=home_mode)
    return app


async def wait_loaded(app, pilot):
    for _ in range(50):
        await pilot.pause(0.02)
        if app.loaded:
            break
    await pilot.pause()


def target_of(app, kind):
    return next((t for t in app.grid.screen_data.targets if t[3] == kind), None)


async def test_click_answer_on_an_agent_card_deep_links_its_question(fakes, monkeypatch):
    monkeypatch.setenv("FAKE_INBOX_VERSION", "0.6.0")
    app = await started()
    async with app.run_test(size=(160, 45)) as pilot:
        await wait_loaded(app, pilot)
        await pilot.press("right", "down", "down", "right")  # PR #48: the asking agent, its card expanded
        t = target_of(app, "answer")
        assert t and t[4] == ("w1:p1", "ref:#48", "c1d2e3f")
        app.arrived = {k: v - 5 for k, v in app.arrived.items()}  # the first paint, a while later
        app.armed_at = __import__("time").monotonic() + 30  # as right after a background change
        await pilot.click(app.grid, offset=(t[0], t[2]))  # not armed: ignored
        assert app.after_exit is None
        app.armed_at = 0
        await pilot.click(app.grid, offset=(t[0] + 1, t[2]))
        await pilot.pause()
    app.after_exit()
    assert fakes.opens()[-1][7:10] == ["open", "--question", "Q2"]


async def test_click_on_the_orchestrator_mark_opens_its_pane_group(fakes, monkeypatch):
    monkeypatch.setenv("FAKE_INBOX_VERSION", "0.6.0")
    app = await started(home_mode=True)
    async with app.run_test(size=(160, 45)) as pilot:
        await wait_loaded(app, pilot)
        app.armed_at = 0
        app.arrived = {k: v - 5 for k, v in app.arrived.items()}  # the first paint's targets, a while later
        t = target_of(app, "answer-orch")
        assert t and t[4] == ("w1:p1",)
        await pilot.click(app.grid, offset=(t[0], t[2]))
        for _ in range(40):
            await pilot.pause(0.05)
            if fakes.opens():
                break
        assert app.is_running  # home tab: the inbox opens over it
    assert fakes.opens()[-1][7:10] == ["open", "--pane", "w1:p1"]


async def test_a_click_on_a_vanished_identity_does_nothing(fakes, monkeypatch):
    monkeypatch.setenv("FAKE_INBOX_VERSION", "0.6.0")
    app = await started()
    async with app.run_test(size=(160, 45)) as pilot:
        await wait_loaded(app, pilot)
        await pilot.press("right", "down", "down", "right")
        t = target_of(app, "answer")
        app.armed_at = 0
        sw = demo_swarm()
        sw.orchestrators[0].threads[2].agents = []  # the asking agent is gone
        app.swarm = sw  # model swapped, screen not yet repainted: the stale target must not act
        app.click_target(t[3], t[4])
        assert app.after_exit is None and app.is_running



# -- review of #6 -------------------------------------------------------------------------

def test_r6_l1_not_installed_is_rechecked(fakes, monkeypatch):
    inbox_link.open_inbox(question=2)  # not installed: plain
    monkeypatch.setenv("FAKE_INBOX_VERSION", "0.6.0")  # installed meanwhile
    inbox_link.open_inbox(question=2)
    assert fakes.opens() == []  # within the re-check interval: still plain
    monkeypatch.setattr(inbox_link, "RECHECK_SECONDS", 0)
    inbox_link.open_inbox(question=2)
    assert len(fakes.opens()) == 1


def test_r6_l2_panoramas_context_does_not_leak(fakes, monkeypatch):
    monkeypatch.setenv("FAKE_INBOX_VERSION", "0.6.0")
    monkeypatch.setenv("HERDR_PLUGIN_CONTEXT_JSON", '{"focused_pane_id": "w1:p9"}')
    monkeypatch.setenv("HERDR_PANE_ID", "w1:p9")
    monkeypatch.setenv("HERDR_PLUGIN_STATE_DIR", "/x/panorama-state")
    inbox_link.open_inbox(pane="w1:p1")
    call = fakes.opens()[-1]
    assert call[call.index("LEAKED") + 1] == []


@pytest.mark.parametrize("rc,out,plain", [(1, "Traceback: something after the popup opened", 0),
                                          (2, "error: unrecognized arguments: --question", 1),
                                          (1, "could not open the inbox popup: herdr down", 1)])
def test_r6_l3_no_second_popup_after_an_unclear_failure(fakes, monkeypatch, rc, out, plain):
    monkeypatch.setenv("FAKE_INBOX_VERSION", "0.6.0")
    monkeypatch.setenv("FAKE_INBOX_RC", str(rc))
    monkeypatch.setenv("FAKE_INBOX_OUT", out)
    inbox_link.open_inbox(question=2)
    assert len(fakes.plain()) == plain


async def test_r6_m1_a_target_that_moved_in_the_last_refresh_ignores_clicks(fakes, monkeypatch):
    from herdr_panorama.model import Orchestrator
    monkeypatch.setenv("FAKE_INBOX_VERSION", "0.6.0")
    app = await started(home_mode=True)
    async with app.run_test(size=(160, 45)) as pilot:
        await wait_loaded(app, pilot)
        app.armed_at = 0
        app.apply(demo_swarm())  # a refresh where nothing moved
        t = target_of(app, "answer-orch")
        sw = demo_swarm()
        sw.orchestrators.insert(0, Orchestrator(pane_id="w1:p0", tab_id="w1:t0", tab_label="New one",
                                                session_id="s0", herdr_status="idle"))
        app.apply(sw)  # the selected card is unchanged (no global re-arm), but every ?N moved down
        assert app.armed()
        moved = target_of(app, "answer-orch")
        assert moved[2] == t[2] + 5
        await pilot.click(app.grid, offset=(moved[0], moved[2]))
        await pilot.pause(0.2)
        assert fakes.opens() == []  # moved less than a second ago: ignored
        app.arrived = {k: v - 5 for k, v in app.arrived.items()}  # a second later
        await pilot.click(app.grid, offset=(moved[0], moved[2]))
        for _ in range(40):
            await pilot.pause(0.05)
            if fakes.opens():
                break
    assert fakes.opens()[-1][7:10] == ["open", "--pane", "w1:p1"]


# -- --no-hold (herdr-inbox >= 0.6.1) ------------------------------------------------------

def test_no_hold_is_passed_when_the_inbox_lists_it(fakes, monkeypatch):
    """Panorama only opens the inbox on an explicit `a` or click, so the popup is not held."""
    monkeypatch.setenv("FAKE_INBOX_VERSION", "0.6.1")
    monkeypatch.setenv("FAKE_INBOX_HELP_NOHOLD", "1")
    inbox_link.open_inbox(question=2)
    inbox_link.open_inbox(pane="w1:p3")
    assert [c[7:] for c in fakes.opens()] == [
        ["open", "--question", "Q2", "--no-hold", "LEAKED", [], "ENV", "leavemyyard.herdr-inbox", str(fakes.root)],
        ["open", "--pane", "w1:p3", "--no-hold", "LEAKED", [], "ENV", "leavemyyard.herdr-inbox", str(fakes.root)]]
    helps = [c for c in fakes.calls("uv") if "--help" in c]
    assert len(helps) == 1  # probed once per inbox version


def test_an_older_inbox_gets_no_no_hold(fakes, monkeypatch):
    monkeypatch.setenv("FAKE_INBOX_VERSION", "0.6.0")  # deep links, but no --no-hold yet
    inbox_link.open_inbox(question=2)
    assert "--no-hold" not in fakes.opens()[0]
    assert fakes.opens()[0][7:10] == ["open", "--question", "Q2"]


def test_a_failed_help_probe_means_no_no_hold(fakes, monkeypatch):
    monkeypatch.setenv("FAKE_INBOX_VERSION", "0.6.1")
    calls = []

    def inbox(root, *a, timeout=20.0):
        calls.append(a)
        return (1, "boom --no-hold") if "--help" in a else (0, "")  # the probe failed
    monkeypatch.setattr(inbox_link, "_inbox", inbox)
    inbox_link.open_inbox(question=2)
    assert calls[-1] == ("open", "--question", "Q2")


def test_the_help_text_is_read_once_for_both_checks(fakes, monkeypatch):
    monkeypatch.setenv("FAKE_INBOX_VERSION", "0.5.9")  # older version: deep links probed by help
    monkeypatch.setenv("FAKE_INBOX_HELP_PANE", "1")
    monkeypatch.setenv("FAKE_INBOX_HELP_NOHOLD", "1")
    inbox_link.open_inbox(question=2)
    assert "--no-hold" in fakes.opens()[0]
    assert len([c for c in fakes.calls("uv") if "--help" in c]) == 1
