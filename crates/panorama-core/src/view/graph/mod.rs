//! The wide graph, design 2a (Python `render.py`'s wide layout, from 120 columns): its
//! columns of cards. Each column is drawn on its own tall canvas: the orchestrators
//! ([`orchs`]), the selected orchestrator's threads ([`threads`]), and the selected thread's
//! agents by depth ([`agent_card`]). The page around them (the tree's placement, the links
//! between the columns, the sideways scroll and the chrome) joins in P4 PR 4b.

pub mod agent_card;
pub mod column;
pub mod orchs;
pub mod thread_card;
pub mod threads;
