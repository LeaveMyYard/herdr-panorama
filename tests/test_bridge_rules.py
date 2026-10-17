"""The rules both plugins must share (plan docs/plans/rust-rewrite.md §1.3 rows 3-5, D6), pinned
as tables so the inbox and the Rust port can copy them case for case:

- one session-id validator (``discovery.is_uuid``) and its canonical form;
- the continuation rule (D6): a ``continued-in`` record of the main transcript (not a sidechain)
  naming another session id (compared in canonical lowercase) is cleared only by a later
  ``user`` or ``assistant`` record, never by bookkeeping records, known or new;
- "is this process Claude Code?" (``hooks.is_claude``).

Every transcript record is a captured template (tests/fixtures/templates.json, scenario.py).
"""

import json

import pytest

from herdr_panorama import hooks, machines
from herdr_panorama.scan import discovery
from herdr_panorama.scan.session import MAIN, Session

from .scenario import BASE, Project, iso, tpl

SID = "55555555-5555-4555-8555-555555555555"
NEXT = "66666666-6666-4666-8666-666666666666"
HEX = "abcdef01-2345-4abc-8def-0123456789ab"  # has letters: HEX.upper() != HEX

# -- one session-id validator --------------------------------------------------------------

VALID = [
    (SID, SID),
    ("0a1b2c3d-4e5f-4a6b-8c7d-9e0f1a2b3c4d", "0a1b2c3d-4e5f-4a6b-8c7d-9e0f1a2b3c4d"),
    ("0A1B2C3D-4E5F-4A6B-8C7D-9E0F1A2B3C4D", "0a1b2c3d-4e5f-4a6b-8c7d-9e0f1a2b3c4d"),  # any case, canonical lower
    ("0a1B2c3D-4e5F-4a6B-8c7D-9e0F1a2B3c4D", "0a1b2c3d-4e5f-4a6b-8c7d-9e0f1a2b3c4d"),
]

INVALID = [
    "",
    " " + SID,
    SID + " ",
    SID + "\n",  # the whole string, no trailing newline
    "\t" + SID,
    "5555555-5555-4555-8555-5555555555555",  # 7-4-4-4-13
    "55555555-5555-4555-8555-55555555555",  # short
    "55555555-5555-4555-8555-5555555555555",  # long
    "555555555555455585555555555555555",  # no dashes
    "55555555_5555_4555_8555_555555555555",
    "g5555555-5555-4555-8555-555555555555",  # not hex
    "../" + SID,
    SID + "/..",
    "..",
    "../../outside",
    "/etc/passwd",
    "a/b",
    "a\\b",
    "55555555-5555-4555-8555-55555555555/",
    "55555555-5555-4555-8555-55555555555\\",
    "*",
    "5555555*-5555-4555-8555-555555555555",
    "55555555-5555-4555-8555-55555555555?",
    "[5]555555-5555-4555-8555-555555555555",
    "agent-a1",
    None,
    123,
]


@pytest.mark.parametrize("sid,canonical", VALID)
def test_a_session_id_in_any_case_is_valid_and_canonical_is_lowercase(sid, canonical):
    assert discovery.is_uuid(sid)
    assert machines.valid_session(sid)
    assert discovery.canonical_session_id(sid) == canonical


@pytest.mark.parametrize("sid", INVALID)
def test_anything_else_is_not_a_session_id(sid):
    assert not discovery.is_uuid(sid)
    assert not machines.valid_session(sid)
    assert discovery.canonical_session_id(sid) is None


def test_machines_has_no_validator_of_its_own():
    """One validator: machines delegates to discovery (the drift in §1.3 row 4 was four regexes)."""
    assert not hasattr(machines, "_SESSION")


@pytest.mark.parametrize("sid", [SID, HEX])
def test_an_uppercase_id_finds_its_lowercase_transcript(tmp_path, sid):
    """Claude Code names transcripts by the lowercase id; an id handed over in another case is
    the same session."""
    assert HEX.upper() != HEX
    proj = Project(tmp_path / "projects")
    m = proj.main(sid)
    m.text(BASE, "hi")
    m.write()
    assert discovery.find_root(sid.upper(), None, str(proj.root)) == str(m.path)


def test_a_continuation_named_in_uppercase_is_found_and_followed(tmp_path):
    from herdr_panorama.collect import Collector
    proj = Project(tmp_path / "projects")
    old = old_session(tmp_path)
    continued(old, BASE + 10, HEX.upper())
    old.write()
    new = proj.main(HEX)
    new.prompt(BASE + 11, "next")
    new.write()
    c = Collector(root=str(proj.root), now=lambda: BASE + 60, done_window=6 * 3600, procs=False)
    ids, sess = c.live_session(SID, None)
    assert ids == [SID, HEX] and sess is not None and sess.root == str(new.path)
    ids, sess = c.live_session(HEX.upper(), None)  # herdr naming the new session in uppercase
    assert sess is not None and sess.root == str(new.path)


# -- the continuation rule (D6) ------------------------------------------------------------

UNKNOWN = "made-up-bookkeeping"  # a record type Claude Code may add tomorrow


def bookkeeping(m, kind: str) -> None:
    """A captured bookkeeping record, as Claude Code writes it (undated); ``UNKNOWN``: the
    captured ``mode`` record under a type this code has never seen."""
    rec = tpl("mode" if kind == UNKNOWN else kind)
    if kind == UNKNOWN:
        rec["type"] = UNKNOWN
    rec["sessionId"] = m.session
    m.raw(json.dumps(rec, separators=(",", ":")))


def continued(m, t: float, nxt: str = NEXT) -> None:
    rec = tpl("continued_in")
    rec.update(timestamp=iso(t), sessionId=m.session, continuedInSessionId=nxt)
    m.raw(json.dumps(rec, separators=(",", ":")))


def load(m) -> Session:
    s = Session(m.session, str(m.path))
    s.update()
    return s


def old_session(tmp_path):
    proj = Project(tmp_path / "projects")
    m = proj.main(SID)
    m.prompt(BASE, "work")
    m.text(BASE + 1, "working")
    return m


@pytest.mark.parametrize("kinds", [
    [], ["last_prompt"], ["cost_state"], ["mode"], [UNKNOWN],
    ["last_prompt", "cost_state", "mode", UNKNOWN],
])
def test_bookkeeping_after_continued_in_does_not_clear_it(tmp_path, kinds):
    m = old_session(tmp_path)
    continued(m, BASE + 10)
    for kind in kinds:
        bookkeeping(m, kind)
    m.write()
    assert load(m).continued_in == NEXT


@pytest.mark.parametrize("kind", ["user", "assistant"])
@pytest.mark.parametrize("dt,cleared", [(+1, True), (0, False), (-1, False)])
def test_only_a_later_conversation_record_clears_it(tmp_path, kind, dt, cleared):
    """``claude --resume <old>``: the conversation goes on in the old file, so it is live again.
    A conversation record at or before the continued-in's time is not a resume."""
    m = old_session(tmp_path)
    continued(m, BASE + 10)
    bookkeeping(m, "last_prompt")
    bookkeeping(m, "cost_state")
    (m.prompt if kind == "user" else m.text)(BASE + 10 + dt, "go on")
    m.write()
    s = load(m)
    assert (s.continued_in is None) == cleared
    assert s.continued_in in (None, NEXT)


def test_a_resume_appended_later_clears_it_on_the_next_read(tmp_path):
    m = old_session(tmp_path)
    continued(m, BASE + 10)
    bookkeeping(m, "mode")
    bookkeeping(m, UNKNOWN)
    m.write()
    s = load(m)
    assert s.continued_in == NEXT
    m.prompt(BASE + 20, "go on")
    m.write()
    s.update()
    assert s.continued_in is None


@pytest.mark.parametrize("nxt", ["../../outside", "*", "/etc/passwd", "", SID, SID.upper(), NEXT + "\n"])
def test_a_continuation_that_is_not_another_session_id_is_ignored(tmp_path, nxt):
    """Not a UUID, or this session's own id (in any case): never followed."""
    m = old_session(tmp_path)
    continued(m, BASE + 10, nxt)
    m.write()
    assert load(m).continued_in is None


def test_a_continuation_in_another_case_is_kept_canonical(tmp_path):
    m = old_session(tmp_path)
    continued(m, BASE + 10, HEX.upper())
    m.write()
    assert load(m).continued_in == HEX


@pytest.mark.parametrize("own,nxt", [(HEX, HEX.upper()), (HEX.upper(), HEX), (HEX, HEX.title())])
def test_a_continuation_to_its_own_id_in_another_case_is_ignored(tmp_path, own, nxt):
    proj = Project(tmp_path / "projects")
    m = proj.main(own)
    m.prompt(BASE, "work")
    continued(m, BASE + 10, nxt)
    m.write()
    assert load(m).continued_in is None


def test_a_sidechain_continuation_is_ignored(tmp_path):
    """A continued-in record marked ``isSidechain: true`` is never the main conversation's, even
    in the main transcript."""
    m = old_session(tmp_path)
    rec = tpl("continued_in")
    rec.update(timestamp=iso(BASE + 10), sessionId=SID, isSidechain=True, continuedInSessionId=NEXT)
    m.raw(json.dumps(rec, separators=(",", ":")))
    m.write()
    assert load(m).continued_in is None


@pytest.mark.parametrize("ignored", ["sidechain", "own_id", "not_a_uuid"])
def test_a_resume_before_an_ignored_continuation_in_the_same_batch_still_counts(tmp_path, ignored):
    """continued-in, then the resume, then a continued-in that is ignored, all in one read: the
    resume scan starts after the last ACCEPTED continued-in, so the resume still clears it."""
    m = old_session(tmp_path)
    continued(m, BASE + 10)
    m.prompt(BASE + 20, "go on")
    rec = tpl("continued_in")
    rec.update(timestamp=iso(BASE + 30), sessionId=SID,
               continuedInSessionId={"own_id": SID.upper(), "not_a_uuid": "../x"}.get(ignored, NEXT))
    if ignored == "sidechain":
        rec["isSidechain"] = True
    m.raw(json.dumps(rec, separators=(",", ":")))
    m.write()
    assert load(m).continued_in is None


def undated_continued(m, nxt: str = NEXT) -> None:
    """A continued-in record without a timestamp: it takes the time of the record before it."""
    rec = tpl("continued_in")
    rec.pop("timestamp")
    rec.update(sessionId=m.session, continuedInSessionId=nxt)
    m.raw(json.dumps(rec, separators=(",", ":")))


@pytest.mark.parametrize("kind", ["user", "assistant"])
@pytest.mark.parametrize("dt,cleared", [(+1, True), (0, False), (-1, False)])
def test_an_undated_continuation_takes_the_previous_dated_records_time(tmp_path, kind, dt, cleared):
    """Its time is that of the last DATED record of any type before it (here the assistant text
    at BASE + 5, a record the scanner's prefilter does not parse); undated bookkeeping between
    them does not move it."""
    proj = Project(tmp_path / "projects")
    m = proj.main(SID)
    m.prompt(BASE, "work")
    m.prompt(BASE + 1, "more")
    m.text(BASE + 5, "working")
    bookkeeping(m, "mode")
    undated_continued(m)
    bookkeeping(m, "cost_state")
    (m.prompt if kind == "user" else m.text)(BASE + 5 + dt, "go on")
    m.write()
    assert (load(m).continued_in is None) == cleared


@pytest.mark.parametrize("dt,cleared", [(+1, True), (0, False)])
def test_an_undated_continuation_takes_a_dated_record_from_an_earlier_read(tmp_path, dt, cleared):
    """The dated record was read in an earlier batch than the undated continued-in."""
    proj = Project(tmp_path / "projects")
    m = proj.main(SID)
    m.prompt(BASE, "work")
    m.text(BASE + 5, "working")
    m.write()
    s = load(m)
    undated_continued(m)
    m.write()
    s.update()
    assert s.continued_in == NEXT
    m.text(BASE + 5 + dt, "go on")
    m.write()
    s.update()
    assert (s.continued_in is None) == cleared


def test_an_undated_continuation_with_no_time_before_it_is_cleared_by_any_conversation(tmp_path):
    """Nothing dated before it: no time to compare with, so any later user/assistant record is a
    resume; bookkeeping still is not."""
    proj = Project(tmp_path / "projects")
    m = proj.main(SID)
    undated_continued(m)
    bookkeeping(m, "last_prompt")
    bookkeeping(m, UNKNOWN)
    m.write()
    s = load(m)
    assert s.continued_in == NEXT
    m.text(BASE, "go on")
    m.write()
    s.update()
    assert s.continued_in is None


def test_a_continuation_from_a_subagent_file_is_ignored(tmp_path):
    """Only the main transcript's continued-in counts."""
    s = Session(SID, str(tmp_path / f"{SID}.jsonl"))
    rec = tpl("continued_in")
    rec.update(sessionId=SID, continuedInSessionId=NEXT)
    s.apply("a1", rec, BASE)
    assert s.continued_in is None
    s.apply(MAIN, rec, BASE)
    assert s.continued_in == NEXT


# -- is this process Claude Code? ----------------------------------------------------------

CLAUDE = [
    ("claude", []),  # native install, by command name
    ("claude", ["claude", "--resume"]),
    ("2.1.301", ["/home/u/.local/share/claude/versions/2.1.301"]),  # versioned binary: comm is the version
    ("2.1.301", ["/home/u/.local/share/claude/versions/2.1.301", "--continue"]),
    ("MainThread", ["/home/u/.local/bin/claude"]),  # a symlink named claude
    ("node", ["node", "/usr/lib/node_modules/@anthropic-ai/claude-code/cli.js"]),
    ("node", ["/usr/bin/node", "/home/u/.npm-global/lib/node_modules/@anthropic-ai/claude-code/cli.js", "-p"]),
    ("node", ["node", "--no-warnings", "/opt/x/@anthropic-ai/claude-code/cli.js"]),  # one option before it
    ("node22", ["node22", "/x/@anthropic-ai/claude-code/cli.mjs"]),
    # under npx: the package's own entry script, below the launcher
    ("node", ["/home/u/.nvm/versions/node/v24.12.0/bin/node",
              "/home/u/.npm/_npx/0123456789abcdef/node_modules/@anthropic-ai/claude-code/cli.js"]),
    # Claude Code sets its process title: captured as comm "claude", cmdline "claude"
    ("claude", ["claude"]),
]

# The npx launcher: node running npm's npx-cli.js with the package NAME (the path is the real
# one, `readlink -f $(which npx)`); after start-up npm retitles it, captured from a running
# `npx <pkg>` as comm "npm exec @model" (15 bytes) and cmdline "npm exec <pkg>" plus empty
# padding args. The package name here is substituted (derived, no claude-code npx run here).
NPX = "/home/u/.nvm/versions/node/v24.12.0/lib/node_modules/npm/bin/npx-cli.js"
NPX_LAUNCHER = [
    ("node", ["/home/u/.nvm/versions/node/v24.12.0/bin/node", NPX, "@anthropic-ai/claude-code"]),
    ("node", ["node", NPX, "-y", "@anthropic-ai/claude-code"]),
    ("npm exec @anthr", ["npm exec @anthropic-ai/claude-code", "", "", ""]),
]

NOT_CLAUDE = [
    ("bash", ["bash", "-c", "claude"]),  # a shell that will start claude is not claude
    ("python3", ["python3", "-m", "claude"]),
    ("claude-code", ["claude-code"]),
    ("claudette", ["/usr/bin/claudette"]),
    ("node", ["node", "server.js"]),
    ("node", ["node", "a", "b", "/x/@anthropic-ai/claude-code/cli.js"]),  # only the first two args
    ("node", ["node", "@anthropic-ai/claude-code"]),  # a bare package name is not the entry script
    ("node", ["node", "/x/@anthropic-ai/claude-code"]),
    ("node", ["node", "/home/u/src/claude-code/cli.js"]),  # some other claude-code checkout
    ("node", ["node", "/x/@anthropic-ai/claude-code/cli.js.map"]),
    ("node", ["node", "/x/@anthropic-ai/claude-code-router/cli.js"]),
    ("node", []),
    ("uv", ["uv", "run", "/x/@anthropic-ai/claude-code/cli.js"]),  # not node
    ("zsh", ["/home/u/.local/share/claude-versions/2.1.301"]),
    ("", []),
]


@pytest.mark.parametrize("comm,argv", CLAUDE)
def test_is_claude(comm, argv):
    assert hooks.is_claude(comm, argv)


@pytest.mark.parametrize("comm,argv", NOT_CLAUDE + NPX_LAUNCHER)
def test_is_not_claude(comm, argv):
    assert not hooks.is_claude(comm, argv)


def test_a_launcher_above_claude_gives_exactly_one_claude(monkeypatch):
    """npx above the real cli: walking up from a hook finds the cli, and the whole chain holds
    one Claude (the launcher is not a second, outer one)."""
    import os
    tree = {100: (90, "python3", "s1", ["python3", "-I", "hooks.py"]),
            90: (80, "sh", "s2", ["sh", "-c", "..."]),
            80: (70, "node", "s3", ["/home/u/.nvm/versions/node/v24.12.0/bin/node",
                                     "/home/u/.npm/_npx/0123456789abcdef/node_modules/@anthropic-ai/claude-code/cli.js"]),
            70: (60, "sh", "s4", ["sh", "-c", "claude"]),
            60: (50, "node", "s5", ["/home/u/.nvm/versions/node/v24.12.0/bin/node", NPX, "@anthropic-ai/claude-code"]),
            50: (1, "zsh", "s6", ["-zsh"])}
    monkeypatch.setattr(hooks, "_stat", lambda pid: tree[pid][:3] if pid in tree else None)
    monkeypatch.setattr(hooks, "_cmdline", lambda pid: tree[pid][3] if pid in tree else [])
    monkeypatch.setattr(os, "getppid", lambda: 100)
    assert hooks.claude_process() == (80, "s3")
    assert [pid for pid, (_, comm, _, argv) in tree.items() if hooks.is_claude(comm, argv)] == [80]
    tree[60] = (50, "npm exec @anthr", "s5", ["npm exec @anthropic-ai/claude-code", "", ""])  # retitled
    assert [pid for pid, (_, comm, _, argv) in tree.items() if hooks.is_claude(comm, argv)] == [80]
    tree[80] = (70, "bash", "s3", ["bash"])  # no cli below the launcher: no Claude at all
    assert hooks.claude_process() is None


def test_herdr_claude_pid_skips_the_launcher(monkeypatch):
    """herdr.claude_pid (pane process-info) applies the same rule to the pane's foreground."""
    from herdr_panorama import herdr
    procs = [{"pid": 60, "name": "node", "argv": ["node", NPX, "@anthropic-ai/claude-code"]},
             {"pid": 80, "name": "node", "argv": ["node", "/x/node_modules/@anthropic-ai/claude-code/cli.js"]}]
    monkeypatch.setattr(herdr, "call", lambda *a, **k: ({"process_info": {"foreground_processes": procs}}, None))
    assert herdr.claude_pid("w1:p1") == 80
    monkeypatch.setattr(herdr, "call", lambda *a, **k: ({"process_info": {"foreground_processes": procs[:1]}}, None))
    assert herdr.claude_pid("w1:p1") is None
