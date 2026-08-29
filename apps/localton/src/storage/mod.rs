//! Persistent files that describe and track a local network.
//!
//! The storage layer defines the state-directory layout, immutable network
//! manifest, editable settings, and live runtime status. Callers use the types
//! re-exported here without depending on their JSON file organization.

mod full_node_bootstrap;
mod layout;
mod runtime_state;
mod settings;

pub(crate) use full_node_bootstrap::*;
pub(crate) use layout::*;
pub(crate) use runtime_state::*;
pub(crate) use settings::*;
