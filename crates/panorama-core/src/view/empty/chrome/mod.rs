//! The graph's chrome (Python `render.header`, `legend`, `keys`), which the empty pages wear.

mod header;
mod keys;
mod legend;

pub use header::header;
pub use keys::keys;
pub use legend::legend;
