"""Issue #13: the docked panel's footer names the open key as bound in herdr's config.toml,
in a short form when the full one does not fit; the default when it cannot be read."""

import os

import pytest

from herdr_panorama import openkey, render

from .design_dock import design_swarm
from .test_dock_panel import design_view


def herdr_config(text: str) -> None:
    d = os.path.join(os.environ["XDG_CONFIG_HOME"], "herdr")
    os.makedirs(d, exist_ok=True)
    path = os.path.join(d, "config.toml")
    with open(path, "w") as fh:
        fh.write(text)
    st = os.stat(path)
    os.utime(path, (st.st_atime, st.st_mtime + 1))  # a fresh mtime: the cache re-reads


BOUND = '''
[[keys.command]]
key = "ctrl+alt+p"
type = "plugin_action"
command = "leavemyyard.herdr-panorama.open"
description = "swarm view"
'''


@pytest.mark.parametrize("key,short", [("prefix+space", "pfx+spc"), ("ctrl+alt+p", "c-a-p"),
                                       ("prefix+alt+g", "pfx+a-g"), ("ctrl+shift+enter", "c-s-ret"), ("f12", "f12")])
def test_the_short_form(key, short):
    assert openkey.short(key) == short


def test_the_bound_key_is_read_and_re_read_on_change():
    assert openkey.current() == openkey.DEFAULT  # no config: today's default
    herdr_config("[keys]\nprefix = \"ctrl+b\"\n" + BOUND)
    assert openkey.current() == ("ctrl+alt+p", "c-a-p")
    herdr_config(BOUND.replace("ctrl+alt+p", "f12"))
    assert openkey.current() == ("f12", "f12")
    herdr_config("[[keys.command]]\nkey = \"ctrl+g\"\ntype = \"popup\"\ncommand = \"lazygit\"\n")  # someone else's
    assert openkey.current() == openkey.DEFAULT
    herdr_config("not toml [[[")
    assert openkey.current() == openkey.DEFAULT


@pytest.mark.parametrize("key,shown", [(("ctrl+alt+p", "c-a-p"), "c-a-p"), (("f12", "f12"), "f12"),
                                       (("ctrl+p", "c-p"), "ctrl+p")])
def test_the_footer_shows_it_whole_when_it_fits_else_short(key, shown):
    v = design_view(4)
    v.open_key = key
    foot = render.render(design_swarm(), v, 46, 45).text().splitlines()[-1].strip()
    assert foot == f"↑↓ move ret open a answer n new {shown} back"


def test_the_default_footer_is_unchanged():
    v = design_view(4)
    assert v.open_key == openkey.DEFAULT
    assert render.render(design_swarm(), v, 46, 45).text().splitlines()[-1].strip() == \
        "↑↓ move ret open a answer n new pfx+spc back"
