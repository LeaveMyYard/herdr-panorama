//! The panel's notices (Python's Textual `notify`): toasts stacked in the bottom-right corner
//! over the view, newest at the bottom, each until its time is up; a click on one dismisses
//! it. Drawn as Textual draws them (its `Toast` and `ToastRack` CSS, `textual-dark` colours):
//! 60 cells wide at most half the rack, a left bar in the severity's colour, a cell of
//! padding all round, the title in bold above the text.

use super::input::Toast;
use panorama_core::view::{Rgb, Screen};
use std::time::{Duration, Instant};

/// Textual's `$panel-lighten-1`, `$foreground`, `$success`/`$text-success`, `$warning`/`$text-warning`.
const BG: Rgb = Rgb::hex(0x343f49);
const TEXT: Rgb = Rgb::hex(0xe0e0e0);
const INFO: (Rgb, Rgb) = (Rgb::hex(0x4ebf71), Rgb::hex(0x8ad4a1));
const WARNING: (Rgb, Rgb) = (Rgb::hex(0xfea62b), Rgb::hex(0xffc473));
/// `Toast { width: 60; max-width: 50% }`.
const WIDTH: i32 = 60;
/// The rack's vertical scrollbar takes these columns on the right.
const GUTTER: i32 = 2;

/// The toasts shown, each until its time.
#[derive(Debug, Default)]
pub struct Toasts {
    list: Vec<(Toast, Instant)>,
}

/// Where a toast was drawn: rows `y0..y1`, columns `x0..x1`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Placed {
    pub x0: i32,
    pub x1: i32,
    pub y0: i32,
    pub y1: i32,
}

/// `text` in lines of at most `w` cells, as Rich folds it: words greedily, a word longer than
/// a line cut into pieces of `w`.
pub fn fold(text: &str, w: usize) -> Vec<String> {
    let w = w.max(1);
    let mut out: Vec<String> = vec![];
    let mut cur = String::new();
    for word in text.split_whitespace() {
        let mut word: Vec<char> = word.chars().collect();
        let len = |s: &String| s.chars().count();
        if !cur.is_empty() && len(&cur) + 1 + word.len() <= w {
            cur.push(' ');
            cur.extend(word);
            continue;
        }
        if !cur.is_empty() {
            out.push(std::mem::take(&mut cur));
        }
        while word.len() > w {
            out.push(word.drain(..w).collect());
        }
        cur = word.into_iter().collect();
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

impl Toasts {
    /// Shows `toast` from `now` for its seconds.
    pub fn push(&mut self, toast: Toast, now: Instant) {
        let until = now + Duration::from_secs(toast.seconds);
        self.list.push((toast, until));
    }

    /// Drops the toasts whose time is up; whether any went.
    pub fn expire(&mut self, now: Instant) -> bool {
        let before = self.list.len();
        self.list.retain(|(_, until)| *until > now);
        self.list.len() != before
    }

    /// When the next toast's time is up.
    pub fn next_expiry(&self) -> Option<Instant> {
        self.list.iter().map(|(_, until)| *until).min()
    }

    /// Draws the toasts over `s`; where each went, oldest first.
    pub fn draw(&self, s: &mut Screen) -> Vec<Placed> {
        let rack = s.w() - GUTTER;
        let width = WIDTH.min(rack / 2);
        let content = (width - 3).max(1) as usize;
        let mut bottom = s.h() - 1; // the rack keeps the last row free (margin-bottom: 1)
        let mut placed = vec![];
        for (toast, _) in self.list.iter().rev() {
            let body = fold(&toast.text, content);
            let title = toast
                .title
                .as_ref()
                .map(|t| fold(t, content))
                .unwrap_or_default();
            let height = 2 + (title.len() + body.len()) as i32;
            let (x0, y0) = (rack - width, bottom - height);
            let (bar, title_fg) = if toast.warning { WARNING } else { INFO };
            s.fill_bg(x0, y0, width, height, BG);
            for y in y0..bottom {
                s.put(
                    x0 + 1,
                    y,
                    &" ".repeat((width - 1).max(0) as usize),
                    TEXT,
                    Some(BG),
                    false,
                );
                s.put(x0, y, "▌", bar, Some(BG), false);
            }
            for (i, line) in title.iter().enumerate() {
                s.put(x0 + 2, y0 + 1 + i as i32, line, title_fg, Some(BG), true);
            }
            for (i, line) in body.iter().enumerate() {
                s.put(
                    x0 + 2,
                    y0 + 1 + (title.len() + i) as i32,
                    line,
                    TEXT,
                    Some(BG),
                    false,
                );
            }
            placed.push(Placed {
                x0,
                x1: x0 + width,
                y0,
                y1: bottom,
            });
            bottom = y0 - 1; // margin-top: 1
        }
        placed.reverse();
        placed
    }

    /// A click at `(x, y)` on a toast dismisses it; whether one was there. `placed` is what
    /// the last draw returned.
    pub fn dismiss_at(&mut self, x: i32, y: i32, placed: &[Placed]) -> bool {
        let hit = placed
            .iter()
            .position(|p| (p.x0..p.x1).contains(&x) && (p.y0..p.y1).contains(&y));
        match hit {
            Some(i) if i < self.list.len() => {
                self.list.remove(i);
                true
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests;
