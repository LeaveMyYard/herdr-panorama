//! The graph's chrome (Python `render.header`, `legend`, `keys`): the empty pages wear it
//! now, the graph views in P4.

mod header;
mod keys;
mod legend;

pub use header::header;
pub use keys::keys;
pub use legend::legend;
