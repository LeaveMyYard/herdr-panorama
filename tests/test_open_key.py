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
    herdr_config(BOUND.replace("ctrl+alt+p", "prefix+space"))  # the suggested binding: the design's text
    assert openkey.current() == openkey.DEFAULT == ("pfx+spc", "pfx+spc")
    herdr_config("[[keys.command]]\nkey = \"ctrl+g\"\ntype = \"popup\"\ncommand = \"lazygit\"\n")  # someone else's
    assert openkey.current() == openkey.DEFAULT
    herdr_config("not toml [[[")
    assert openkey.current() == openkey.DEFAULT


@pytest.mark.parametrize("key,w,foot", [
    # whole with every hint when it fits
    (("ctrl+p", "c-p"), 46, "↑↓ move ret open a answer n new ctrl+p back"),
    (("f12", "f12"), 46, "↑↓ move ret open a answer n new f12 back"),
    # the operator's answer (Q157): too long with every hint, so "n new" goes and the key stays whole
    (("ctrl+alt+p", "c-a-p"), 46, "↑↓ move ret open a answer ctrl+alt+p back"),
    # still too long without "n new": only then the short form
    (("ctrl+alt+p", "c-a-p"), 40, "↑↓ move ret open a answer c-a-p back"),
    (("ctrl+alt+shift+enter", "c-a-s-ret"), 46, "↑↓ move ret open a answer c-a-s-ret back"),
])
def test_the_footer_keeps_the_key_whole_dropping_n_new_first(key, w, foot):
    v = design_view(4)
    v.open_key = key
    assert render.render(design_swarm(), v, w, 45).text().splitlines()[-1].strip() == foot


def test_the_default_footer_is_unchanged():
    v = design_view(4)
    assert v.open_key == openkey.DEFAULT
    assert render.render(design_swarm(), v, 46, 45).text().splitlines()[-1].strip() == \
        "↑↓ move ret open a answer n new pfx+spc back"
