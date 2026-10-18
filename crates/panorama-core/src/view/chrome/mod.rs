//! The graph's chrome (Python `render.header`, `legend`, `keys`): the graph views and the
//! empty pages wear it.

mod header;
mod keys;
mod legend;

pub use header::header;
pub use keys::keys;
pub use legend::legend;
