//! Where the wide graph's columns go (Python `render.tier`, `window` and `_wide`'s column
//! sums): card widths by the terminal width, which columns fit around the focus, and their
//! x positions. Columns keep their width; only the last one shown may shrink.

/// A depth column's width below 220 columns.
pub const DW: i32 = 44;
/// The narrowest the last depth column shown may shrink to, to fit.
pub const DMIN: i32 = 36;

/// Card widths (orchestrators, threads, agents) by the terminal width `w` (design turn 7):
/// a little wider on big screens, capped, never stretched.
pub fn tier(w: i32) -> (i32, i32, i32) {
    if w >= 300 {
        (36, 64, 60)
    } else if w >= 220 {
        (32, 56, 56)
    } else {
        (28, ((w - 36 - 7 - 2) * 50 / 115).clamp(36, 50), DW)
    }
}

/// The columns shown, `(first, last)`: the focused one `f`, the one before it (its parents)
/// and, when it fits, the one after it (its children); then as many more as fit in `avail`,
/// right first. `hcol` (a sideways scroll) fixes the first one instead.
pub fn window(
    widths: &[i32],
    gaps: &[i32],
    f: usize,
    avail: i32,
    hcol: Option<usize>,
) -> (usize, usize) {
    let n = widths.len();
    let min_of = |i: usize| if i < 2 { widths[i] } else { DMIN };
    let span = |lo: usize, hi: usize| {
        widths[lo..hi].iter().sum::<i32>() + gaps[lo + 1..=hi].iter().sum::<i32>() + min_of(hi)
    };
    let (mut lo, mut hi);
    if let Some(h) = hcol {
        lo = h.min(n - 1);
        hi = lo;
    } else {
        lo = f.saturating_sub(1);
        hi = (n - 1).min((f + 1).max(2));
        while hi > f && span(lo, hi) > avail {
            hi -= 1;
        }
        while lo < f && span(lo, hi) > avail {
            lo += 1;
        }
    }
    while hi + 1 < n && span(lo, hi + 1) <= avail {
        hi += 1;
    }
    if hcol.is_none() {
        while lo > 0 && span(lo - 1, hi) <= avail {
            lo -= 1;
        }
    }
    (lo, hi)
}

/// Column `i`'s name, as the line above the columns says what is off-screen.
pub fn col_label(i: usize) -> String {
    match i {
        0 => "orchestrators".into(),
        1 => "threads".into(),
        _ => format!("depth {}", i - 1),
    }
}

/// The columns of one render: every column's width (the last one shown shrunk to fit), the
/// gaps before them, and the first and last shown with their x.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Columns {
    pub widths: Vec<i32>,
    pub gaps: Vec<i32>,
    pub lo: usize,
    pub hi: usize,
    xs: Vec<i32>,
}

impl Columns {
    /// Lays out `widths` on a `w`-wide screen around the focus column `focus`.
    pub fn place(mut widths: Vec<i32>, w: i32, focus: usize, hcol: Option<usize>) -> Self {
        let dw = tier(w).2;
        let gaps: Vec<i32> = (0..widths.len())
            .map(|i| match i {
                0 => 0,
                1 => 6,
                _ => 7,
            })
            .collect();
        let f = focus.min(widths.len() - 1);
        let (lo, hi) = window(&widths, &gaps, f, w - 6, hcol);
        // scrolled: two more columns on the left for the lines in from there
        let x0 = if lo == 0 { 2 } else { 4 };
        if hi >= 2 {
            // the space left on the right stays empty (operator: a stretched card's sweep
            // runs too long)
            let used = widths[lo..hi].iter().sum::<i32>() + gaps[lo + 1..=hi].iter().sum::<i32>();
            widths[hi] = dw.min(w - 2 - x0 - used);
        }
        let mut xs = vec![0; widths.len()];
        let mut x = x0;
        for i in lo..=hi {
            if i > lo {
                x += gaps[i];
            }
            xs[i] = x;
            x += widths[i];
        }
        Self {
            widths,
            gaps,
            lo,
            hi,
            xs,
        }
    }

    /// Whether column `i` is on screen.
    pub fn shown(&self, i: usize) -> bool {
        (self.lo..=self.hi).contains(&i)
    }

    /// Column `i`'s x (meaningful only for a column shown).
    pub fn x(&self, i: usize) -> i32 {
        self.xs[i]
    }

    /// Column `i`'s width.
    pub fn w(&self, i: usize) -> i32 {
        self.widths[i]
    }

    /// The number of columns, shown or not.
    pub fn len(&self) -> usize {
        self.widths.len()
    }

    /// Whether there are none (never: the orchestrators are always a column).
    pub fn is_empty(&self) -> bool {
        self.widths.is_empty()
    }

    /// Some columns are off-screen, left or right.
    pub fn sideways(&self) -> bool {
        self.lo > 0 || self.hi < self.len() - 1
    }
}

#[cfg(test)]
mod tests;
