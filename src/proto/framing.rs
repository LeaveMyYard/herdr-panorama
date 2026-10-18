//! Lines in and out (Python `protocol.encode` and `LineReader`): one JSON object per line,
//! UTF-8, at most [`MAX_LINE`] bytes. A line that is not a JSON object is skipped (an
//! unknown message is not an error); a longer line ends the connection.

use super::MAX_LINE;
use serde_json::{Map, Value};
use std::fmt;

/// A line over [`MAX_LINE`]: the peer is broken or hostile.
#[derive(Debug, PartialEq, Eq)]
pub struct TooLong(pub usize);

impl fmt::Display for TooLong {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "a line of {} bytes (max {MAX_LINE})", self.0)
    }
}

impl std::error::Error for TooLong {}

/// `msg` as one line, newline included.
pub fn encode(msg: &Value) -> Result<Vec<u8>, TooLong> {
    let mut line = serde_json::to_vec(msg).expect("a JSON value always serializes");
    line.push(b'\n');
    if line.len() > MAX_LINE {
        return Err(TooLong(line.len()));
    }
    Ok(line)
}

/// Bytes in, messages out.
#[derive(Debug, Default)]
pub struct LineReader {
    buf: Vec<u8>,
}

impl LineReader {
    /// The whole messages `data` completes, in order. Linear in the bytes read: each byte is
    /// scanned for a newline once.
    pub fn feed(&mut self, data: &[u8]) -> Result<Vec<Map<String, Value>>, TooLong> {
        let mut out = vec![];
        let mut rest = data;
        while let Some(i) = rest.iter().position(|&b| b == b'\n') {
            let (head, tail) = rest.split_at(i);
            rest = &tail[1..];
            let line = if self.buf.is_empty() {
                head
            } else {
                self.buf.extend_from_slice(head);
                &self.buf[..]
            };
            if line.len() > MAX_LINE {
                return Err(TooLong(line.len()));
            }
            if let Ok(Value::Object(m)) = serde_json::from_slice(line) {
                out.push(m);
            }
            self.buf.clear();
        }
        self.buf.extend_from_slice(rest);
        if self.buf.len() > MAX_LINE {
            return Err(TooLong(self.buf.len()));
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests;
