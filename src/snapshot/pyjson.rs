//! JSON as Python's `json.dumps(value, indent=n, ensure_ascii=False)` writes it: keys in
//! insertion order, floats as Python's `repr`, so `panorama snapshot --json` prints what the
//! Python one prints, byte for byte. (Ported from herdr-agent-kit's test-only
//! `tests/panorama_model/pyjson.rs`.)

use serde_json::Value;

/// A JSON value whose objects keep their key order.
#[allow(missing_docs)]
#[derive(Clone, Debug, PartialEq)]
pub enum J {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    Arr(Vec<J>),
    Obj(Vec<(String, J)>),
}

impl J {
    /// An object of these fields, in order.
    pub fn obj<const N: usize>(fields: [(&str, J); N]) -> J {
        J::Obj(
            fields
                .into_iter()
                .map(|(k, v)| (k.to_string(), v))
                .collect(),
        )
    }

    /// A string.
    pub fn str(s: impl Into<String>) -> J {
        J::Str(s.into())
    }

    /// A string or null.
    pub fn opt_str(s: Option<impl Into<String>>) -> J {
        s.map_or(J::Null, |s| J::Str(s.into()))
    }

    /// A float or null.
    pub fn opt_float(f: Option<f64>) -> J {
        f.map_or(J::Null, J::Float)
    }

    /// A list of integers.
    pub fn ints<I: IntoIterator<Item = i64>>(items: I) -> J {
        J::Arr(items.into_iter().map(J::Int).collect())
    }

    /// Every string value with `from` replaced by `to`.
    pub fn replace_in_strings(self, from: &str, to: &str) -> J {
        match self {
            J::Str(s) => J::Str(s.replace(from, to)),
            J::Arr(a) => J::Arr(
                a.into_iter()
                    .map(|j| j.replace_in_strings(from, to))
                    .collect(),
            ),
            J::Obj(o) => J::Obj(
                o.into_iter()
                    .map(|(k, v)| (k, v.replace_in_strings(from, to)))
                    .collect(),
            ),
            other => other,
        }
    }
}

impl From<&Value> for J {
    /// A parsed value (an object's keys come back sorted: serde_json's map).
    fn from(v: &Value) -> J {
        match v {
            Value::Null => J::Null,
            Value::Bool(b) => J::Bool(*b),
            Value::Number(n) => match n.as_i64() {
                Some(i) => J::Int(i),
                None => J::Float(n.as_f64().expect("a JSON number")),
            },
            Value::String(s) => J::Str(s.clone()),
            Value::Array(a) => J::Arr(a.iter().map(J::from).collect()),
            Value::Object(m) => J::Obj(m.iter().map(|(k, v)| (k.clone(), J::from(v))).collect()),
        }
    }
}

/// Python's `repr` of a float.
pub fn float_repr(f: f64) -> String {
    if f.is_nan() {
        return "NaN".into();
    }
    if f.is_infinite() {
        return if f > 0.0 { "Infinity" } else { "-Infinity" }.into();
    }
    let sci = format!("{f:e}");
    let (mantissa, exp) = sci.split_once('e').expect("an exponent");
    let exp: i32 = exp.parse().expect("an exponent");
    let (sign, mantissa) = match mantissa.strip_prefix('-') {
        Some(m) => ("-", m),
        None => ("", mantissa),
    };
    let digits: String = mantissa.chars().filter(|c| *c != '.').collect();
    if !(-4..16).contains(&exp) {
        let (head, tail) = digits.split_at(1);
        let frac = if tail.is_empty() {
            String::new()
        } else {
            format!(".{tail}")
        };
        let esign = if exp < 0 { '-' } else { '+' };
        return format!("{sign}{head}{frac}e{esign}{:02}", exp.abs());
    }
    let point = exp + 1;
    let body = if point <= 0 {
        format!("0.{}{digits}", "0".repeat((-point) as usize))
    } else if point as usize >= digits.len() {
        format!("{digits}{}.0", "0".repeat(point as usize - digits.len()))
    } else {
        let (a, b) = digits.split_at(point as usize);
        format!("{a}.{b}")
    };
    format!("{sign}{body}")
}

fn string(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

fn write(j: &J, indent: usize, level: usize, out: &mut String) {
    let pad = |out: &mut String, level: usize| {
        out.push('\n');
        out.push_str(&" ".repeat(indent * level));
    };
    match j {
        J::Null => out.push_str("null"),
        J::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        J::Int(i) => out.push_str(&i.to_string()),
        J::Float(f) => out.push_str(&float_repr(*f)),
        J::Str(s) => string(s, out),
        J::Arr(items) if items.is_empty() => out.push_str("[]"),
        J::Obj(items) if items.is_empty() => out.push_str("{}"),
        J::Arr(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                pad(out, level + 1);
                write(item, indent, level + 1, out);
            }
            pad(out, level);
            out.push(']');
        }
        J::Obj(items) => {
            out.push('{');
            for (i, (k, v)) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                pad(out, level + 1);
                string(k, out);
                out.push_str(": ");
                write(v, indent, level + 1, out);
            }
            pad(out, level);
            out.push('}');
        }
    }
}

/// `json.dumps(j, indent=indent, ensure_ascii=False)`.
pub fn dumps(j: &J, indent: usize) -> String {
    let mut out = String::new();
    write(j, indent, 0, &mut out);
    out
}

mod compact;
mod parse;

pub use compact::{Style, dumps_styled};

#[cfg(test)]
mod tests;
