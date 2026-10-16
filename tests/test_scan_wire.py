"""Lenient record parsing and the notice carriers (captured templates)."""

import json

from herdr_panorama.scan import wire

from .scenario import notification, tpl


def test_bad_and_odd_lines_never_raise():
    assert wire.parse_line(b"not json") is None
    assert wire.parse_line(b'{"type":"user","message":') is None
    assert wire.parse_line(b"[1,2]") is None
    assert wire.parse_line(b"\xff\xfe") is None
    rec = tpl("result_bash")
    rec["toolUseResult"] = "Error: string form"  # toolUseResult may be a plain string
    assert wire.blocks(rec) and wire.parse_line(json.dumps(rec)) is not None


def test_notice_from_every_carrier():
    text = notification("a1", "toolu_1", "completed", 7)
    q = tpl("queue_enqueue_notification")
    q["content"] = text
    att = tpl("attachment_queued_command")
    att["attachment"]["prompt"] = text
    sub_att = tpl("sub_attachment_queued_command")
    sub_att["attachment"]["prompt"] = text
    user = tpl("user_prompt")
    user["message"]["content"] = text
    for rec in (q, att, sub_att, user):
        assert wire.parse_notice(wire.notice_text(rec)) == {"task": "a1", "tool_use": "toolu_1", "status": "done",
                                                             "tool_uses": 7}
    q["operation"] = "dequeue"
    assert wire.notice_text(q) is None


def test_notice_statuses():
    st = {s: wire.parse_notice(notification("a", "t", s))["status"] for s in
          ("completed", "failed", "error", "killed", "stopped", "weird")}
    assert st == {"completed": "done", "failed": "fail", "error": "fail", "killed": "cxl", "stopped": "cxl", "weird": None}


def test_human_prompt_rule():
    human = tpl("user_prompt_human")
    human["message"]["content"] = "do it"
    assert wire.is_human_prompt(human)
    human["origin"] = {"kind": "task-notification"}
    assert not wire.is_human_prompt(human)
    legacy = tpl("user_prompt")
    legacy.pop("origin", None)
    legacy["message"]["content"] = "old style"
    assert wire.is_human_prompt(legacy)
    legacy["message"]["content"] = notification("a", "t", "completed")
    assert not wire.is_human_prompt(legacy)


def test_summaries_and_short_paths():
    assert wire.summarize_tool("Bash", {"command": "ls", "description": "List files"}) == "Bash(List files)"
    assert wire.summarize_tool("Read", {"file_path": "/r/proj/src/a.py"}, "/r/proj") == "Read(src/a.py)"
    assert wire.short_path("/r/proj-web/a.py", "/r/proj") == "/r/proj-web/a.py"  # sibling dir untouched
    assert wire.summarize_tool("mcp__srv__tool", {"query": "q"}) == "tool(q)"
    assert wire.summarize_tool("Weird", "not a dict") == "Weird"


def test_wanted_lines_matches_wanted():
    lines = [b'{"type":"assistant","x":"\\"name\\":\\"Bash\\""}', b'{"name":"Agent"}', b'{"role":"user","content":"hi"}',
             b'{"role":"user","content":[]}', b"<task-notification>"]
    assert wire.wanted_lines(lines) == [i for i, ln in enumerate(lines) if wire.wanted(ln)] == [1, 2, 4]
