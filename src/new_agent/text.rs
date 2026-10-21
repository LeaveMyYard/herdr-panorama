//! A one-line text field's value and cursor (Textual's `Input`, as the form used it).

/// A value and its cursor, counted in characters.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Text {
    pub value: String,
    pub cursor: usize,
}

impl Text {
    /// `value` with the cursor at its end.
    pub fn new(value: &str) -> Self {
        Self {
            value: value.to_owned(),
            cursor: value.chars().count(),
        }
    }

    fn at(&self, i: usize) -> usize {
        self.value
            .char_indices()
            .nth(i)
            .map_or(self.value.len(), |(b, _)| b)
    }

    pub fn insert(&mut self, c: char) {
        let at = self.at(self.cursor);
        self.value.insert(at, c);
        self.cursor += 1;
    }

    /// Backspace: the character before the cursor.
    pub fn back(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
            let at = self.at(self.cursor);
            self.value.remove(at);
        }
    }

    /// Delete: the character under the cursor.
    pub fn delete(&mut self) {
        if self.cursor < self.value.chars().count() {
            let at = self.at(self.cursor);
            self.value.remove(at);
        }
    }

    pub fn left(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    pub fn right(&mut self) {
        self.cursor = (self.cursor + 1).min(self.value.chars().count());
    }

    pub fn home(&mut self) {
        self.cursor = 0;
    }

    pub fn end(&mut self) {
        self.cursor = self.value.chars().count();
    }
}
