//! One-line JSON with Python's separators of choice: `json.dumps(j, separators=(item, key),
//! ensure_ascii=ascii)`.

use super::{J, float_repr, string};

/// How a one-line dump is written.
#[derive(Clone, Copy, Debug)]
pub struct Style {
    /// Between items: `", "` (Python's default) or `","`.
    pub item: &'static str,
    /// Between a key and its value: `": "` or `":"`.
    pub key: &'static str,
    /// `ensure_ascii`: every character past 0x7e as `\uXXXX` (a surrogate pair beyond the BMP).
    pub ascii: bool,
}

fn line(j: &J, s: Style, out: &mut String) {
    match j {
        J::Null => out.push_str("null"),
        J::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        J::Int(i) => out.push_str(&i.to_string()),
        J::Float(f) => out.push_str(&float_repr(*f)),
        J::Str(text) => string(text, out),
        J::Arr(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push_str(s.item);
                }
                line(item, s, out);
            }
            out.push(']');
        }
        J::Obj(items) => {
            out.push('{');
            for (i, (k, v)) in items.iter().enumerate() {
                if i > 0 {
                    out.push_str(s.item);
                }
                string(k, out);
                out.push_str(s.key);
                line(v, s, out);
            }
            out.push('}');
        }
    }
}

/// `j` on one line in `style`.
pub fn dumps_styled(j: &J, style: Style) -> String {
    let mut out = String::new();
    line(j, style, &mut out);
    if !style.ascii {
        return out;
    }
    let mut ascii = String::with_capacity(out.len());
    for c in out.chars() {
        if c.is_ascii() && c != '\x7f' {
            ascii.push(c);
        } else {
            let mut units = [0u16; 2];
            for u in c.encode_utf16(&mut units) {
                ascii.push_str(&format!("\\u{u:04x}"));
            }
        }
    }
    ascii
}
