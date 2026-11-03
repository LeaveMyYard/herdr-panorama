//! Runtime JSON rejects duplicate keys at every depth; UI snapshot parsing is unchanged.
use serde::de::{Deserialize, Deserializer, Error, MapAccess, SeqAccess, Visitor};
use serde_json::{Map, Number, Value};
use std::fmt;

struct Unique(Value);
struct Json;
impl<'de> Deserialize<'de> for Unique {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        d.deserialize_any(Json).map(Unique)
    }
}
impl<'de> Visitor<'de> for Json {
    type Value = Value;
    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("unique-key JSON")
    }
    fn visit_unit<E: Error>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }
    fn visit_bool<E: Error>(self, v: bool) -> Result<Value, E> {
        Ok(Value::Bool(v))
    }
    fn visit_i64<E: Error>(self, v: i64) -> Result<Value, E> {
        Ok(v.into())
    }
    fn visit_u64<E: Error>(self, v: u64) -> Result<Value, E> {
        Ok(v.into())
    }
    fn visit_f64<E: Error>(self, v: f64) -> Result<Value, E> {
        if v.fract() == 0.0 && v.abs() <= 9_007_199_254_740_991.0 {
            Ok((v as i64).into())
        } else {
            Number::from_f64(v)
                .map(Value::Number)
                .ok_or_else(|| E::custom("nonfinite runtime number"))
        }
    }
    fn visit_str<E: Error>(self, v: &str) -> Result<Value, E> {
        Ok(Value::String(v.into()))
    }
    fn visit_string<E: Error>(self, v: String) -> Result<Value, E> {
        Ok(Value::String(v))
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut items: A) -> Result<Value, A::Error> {
        let mut out = Vec::new();
        while let Some(Unique(v)) = items.next_element()? {
            out.push(v)
        }
        Ok(Value::Array(out))
    }
    fn visit_map<A: MapAccess<'de>>(self, mut items: A) -> Result<Value, A::Error> {
        let mut out = Map::new();
        while let Some((key, Unique(v))) = items.next_entry::<String, Unique>()? {
            if out.insert(key, v).is_some() {
                return Err(A::Error::custom("duplicate runtime JSON key"));
            }
        }
        Ok(Value::Object(out))
    }
}
pub fn parse(bytes: &[u8]) -> Result<Value, String> {
    serde_json::from_slice::<Unique>(bytes)
        .map(|v| v.0)
        .map_err(|e| e.to_string())
}
pub fn keys(v: &Value, expected: &[&str]) -> Result<(), String> {
    let obj = v.as_object().ok_or("runtime JSON is not an object")?;
    if obj.len() != expected.len() || expected.iter().any(|k| !obj.contains_key(*k)) {
        return Err("unexpected/missing runtime JSON fields".into());
    }
    Ok(())
}
pub fn hex(s: &str, n: usize) -> bool {
    s.len() == n
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
pub fn token(s: &str) -> bool {
    !s.is_empty() && s.len() <= 256 && !s.bytes().any(|b| [0, b'\r', b'\n'].contains(&b))
}
