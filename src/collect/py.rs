//! Python's reading of JSON values and text, where the collector's output depends on it:
//! truthiness, `str()`, `a or b` over times, `int()`, `str.splitlines`, `str.isspace`.

use serde_json::{Map, Value};

/// Python's truth value of a JSON value.
pub fn truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().is_some_and(|f| f != 0.0),
        Value::String(s) => !s.is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
    }
}

/// Python's `str()` of a JSON scalar.
pub fn py_str(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Number(n) => match n.as_i64() {
            Some(i) => i.to_string(),
            None => crate::snapshot::pyjson::float_repr(n.as_f64().unwrap_or(0.0)),
        },
        Value::Bool(b) => if *b { "True" } else { "False" }.into(),
        Value::Null => "None".into(),
        other => other.to_string(),
    }
}

/// `m[key]` when it is a string.
pub fn str_of<'a>(m: &'a Map<String, Value>, key: &str) -> Option<&'a str> {
    m.get(key).and_then(Value::as_str)
}

/// `m[key]` as Python's `str()` when it is truthy.
pub fn truthy_str(m: &Map<String, Value>, key: &str) -> Option<String> {
    m.get(key).filter(|v| truthy(v)).map(py_str)
}

/// Python's `a or b` over optional times (0 is false).
pub fn or_time(a: Option<f64>, b: Option<f64>) -> Option<f64> {
    match a {
        Some(x) if x != 0.0 => Some(x),
        _ => b,
    }
}

/// A time only when it is truthy (not 0).
pub fn truthy_time(t: Option<f64>) -> Option<f64> {
    t.filter(|x| *x != 0.0)
}

/// `str.isspace` for one character.
pub fn py_isspace(c: char) -> bool {
    c.is_whitespace() || ('\x1c'..='\x1f').contains(&c)
}

/// `str.splitlines()`.
pub fn py_splitlines(s: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut iter = s.char_indices().peekable();
    while let Some((i, c)) = iter.next() {
        let end = match c {
            '\r' if matches!(iter.peek(), Some(&(_, '\n'))) => {
                iter.next();
                i + 2
            }
            '\r' | '\n' | '\x0b' | '\x0c' | '\x1c' | '\x1d' | '\x1e' | '\u{85}' | '\u{2028}'
            | '\u{2029}' => i + c.len_utf8(),
            _ => continue,
        };
        out.push(&s[start..i]);
        start = end;
    }
    if start < s.len() {
        out.push(&s[start..]);
    }
    out
}

/// The first `n` characters (`s[:n]`).
pub fn chars(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

/// Python's `int(value)` with a default (`collect._int`).
pub fn py_int(v: Option<&Value>, default: i64) -> i64 {
    match v {
        Some(Value::Bool(b)) => i64::from(*b),
        Some(Value::Number(n)) => n
            .as_i64()
            .or_else(|| {
                n.as_f64()
                    .filter(|f| f.is_finite())
                    .map(|f| f.trunc() as i64)
            })
            .unwrap_or(default),
        Some(Value::String(s)) => s.trim().replace('_', "").parse().unwrap_or(default),
        _ => default,
    }
}

#[cfg(test)]
mod tests;
