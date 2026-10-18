//! Text helpers (Python `render.trunc`, `fit`, `dur`, `ago`, `hhmm`). Lengths are counted in
//! characters, as Python counts them.

fn chars(s: &str) -> usize {
    s.chars().count()
}

/// `s` cut to `n` characters, the last one an ellipsis when it was cut (`n` < 1: just `…`).
pub fn trunc(s: &str, n: i32) -> String {
    if chars(s) as i64 <= i64::from(n) {
        return s.to_owned();
    }
    let keep = (n - 1).max(0) as usize;
    s.chars().take(keep).chain(['…']).collect()
}

/// `s` in exactly `n` cells: cut with an ellipsis, or padded with spaces (`n` < 1: empty).
pub fn fit(s: &str, n: i32) -> String {
    if n <= 0 {
        return String::new();
    }
    let (len, n) = (chars(s), n as usize);
    if len > n {
        s.chars().take(n - 1).chain(['…']).collect()
    } else {
        format!("{s}{}", " ".repeat(n - len))
    }
}

/// `text` in lines of at most `w` cells, broken at spaces (each line then cut to `w`), as
/// Python's `render.wrap` (which splits on single spaces, so runs of spaces keep empty words).
pub fn wrap(text: &str, w: i32) -> Vec<String> {
    let mut out = vec![];
    let mut cur = String::new();
    for wd in text.split(' ') {
        if cur.is_empty() {
            cur = wd.to_owned();
        } else if (chars(&cur) + 1 + chars(wd)) as i64 <= i64::from(w) {
            cur.push(' ');
            cur.push_str(wd);
        } else {
            out.push(std::mem::replace(&mut cur, wd.to_owned()));
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out.iter().map(|l| trunc(l, w)).collect()
}

/// A duration: `42m`, `1h05m` (`None` or negative: `0m`).
pub fn dur(seconds: Option<f64>) -> String {
    let m = (seconds.unwrap_or(0.0).max(0.0) / 60.0).floor() as i64;
    if m >= 60 {
        format!("{}h{:02}m", m / 60, m % 60)
    } else {
        format!("{m}m")
    }
}

/// An idle age: minutes, hours up to 48, then days.
pub fn ago(seconds: f64) -> String {
    let m = (seconds.max(0.0) / 60.0).floor() as i64;
    if m >= 2880 {
        format!("{}d", m / 1440)
    } else if m >= 60 {
        format!("{}h", m / 60)
    } else {
        format!("{m}m")
    }
}

/// `HH:MM` of POSIX time `t` at `utc_offset` seconds east of UTC (`None` or 0: empty, as
/// Python's `hhmm` for no time). The caller looks the offset up; the core reads no clock.
pub fn hhmm(t: Option<f64>, utc_offset: i64) -> String {
    match t {
        Some(t) if t != 0.0 => {
            let local = t.floor() as i64 + utc_offset;
            let minute = local.div_euclid(60).rem_euclid(24 * 60);
            format!("{:02}:{:02}", minute / 60, minute % 60)
        }
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests;
