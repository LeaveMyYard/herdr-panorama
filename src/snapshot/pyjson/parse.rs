//! Reading JSON into a [`J`], keys in file order (Python's `json.loads` keeps them), and the
//! object accessors a read-modify-write needs.

use super::J;
use serde::de::{Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};
use std::fmt;

struct JVisitor;

impl<'de> Visitor<'de> for JVisitor {
    type Value = J;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("JSON")
    }
    fn visit_unit<E>(self) -> Result<J, E> {
        Ok(J::Null)
    }
    fn visit_bool<E>(self, b: bool) -> Result<J, E> {
        Ok(J::Bool(b))
    }
    fn visit_i64<E>(self, i: i64) -> Result<J, E> {
        Ok(J::Int(i))
    }
    fn visit_u64<E>(self, u: u64) -> Result<J, E> {
        Ok(i64::try_from(u).map_or(J::Float(u as f64), J::Int))
    }
    fn visit_f64<E>(self, f: f64) -> Result<J, E> {
        Ok(J::Float(f))
    }
    fn visit_str<E>(self, s: &str) -> Result<J, E> {
        Ok(J::Str(s.to_owned()))
    }
    fn visit_string<E>(self, s: String) -> Result<J, E> {
        Ok(J::Str(s))
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<J, A::Error> {
        let mut items = Vec::new();
        while let Some(v) = seq.next_element()? {
            items.push(v);
        }
        Ok(J::Arr(items))
    }
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<J, A::Error> {
        let mut out: Vec<(String, J)> = Vec::new();
        while let Some((k, v)) = map.next_entry::<String, J>()? {
            // a repeated key: the last value, in the first one's place (Python's dict)
            match out.iter_mut().find(|(key, _)| *key == k) {
                Some(slot) => slot.1 = v,
                None => out.push((k, v)),
            }
        }
        Ok(J::Obj(out))
    }
}

impl<'de> Deserialize<'de> for J {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<J, D::Error> {
        d.deserialize_any(JVisitor)
    }
}

impl J {
    /// Parsed from JSON text, or `None`.
    pub fn parse(text: &str) -> Option<J> {
        serde_json::from_str(text).ok()
    }

    /// An object's value at `key`.
    pub fn get(&self, key: &str) -> Option<&J> {
        match self {
            J::Obj(items) => items.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    /// An object's value at `key`, to change.
    pub fn get_mut(&mut self, key: &str) -> Option<&mut J> {
        match self {
            J::Obj(items) => items.iter_mut().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    /// Set an object's `key`: in its place when it is there, else at the end (Python's
    /// `d[key] = value`). Nothing for a non-object.
    pub fn set(&mut self, key: &str, value: J) {
        if let J::Obj(items) = self {
            match items.iter_mut().find(|(k, _)| k == key) {
                Some(slot) => slot.1 = value,
                None => items.push((key.to_owned(), value)),
            }
        }
    }

    /// The text of a string.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            J::Str(s) => Some(s),
            _ => None,
        }
    }

    /// Whether it is an object.
    pub fn is_obj(&self) -> bool {
        matches!(self, J::Obj(_))
    }
}
