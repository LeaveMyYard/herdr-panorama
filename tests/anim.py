"""A held animation clock for tests and the screenshot scripts: the app's ``anim_clock`` is the
wall clock (tui.SwarmApp.anim_frame), so a fixed, hand-moved clock is what pins a frame."""

from __future__ import annotations

from herdr_panorama.tui import FRAME_SECONDS


class AnimClock:
    """A fake anim_clock: seconds, held still until moved (``t``)."""

    def __init__(self, t: float = 0.0):
        self.t = t

    def __call__(self) -> float:
        return self.t


def at_frame(app, frame: float) -> AnimClock:
    """Hold ``app``'s animation at design frame ``frame``; the next repaint or tick draws it."""
    app.anim_clock = clock = AnimClock(frame * FRAME_SECONDS)
    return clock
