//! Typed interfaces to the official TON programs used by Localton.
//!
//! Each module owns one executable's semantic contract and the production
//! adapter for the pinned release. Bootstrap and operation workflows depend on
//! these traits so command-line syntax, output parsing, and release quirks do not
//! leak into network orchestration.

pub(crate) mod create_state;
pub(crate) mod dht_server;
pub(crate) mod fift;
pub(crate) mod lite_client;
pub(crate) mod random_id;
pub(crate) mod types;
pub(crate) mod validator_console;
pub(crate) mod validator_engine;
pub(crate) mod validator_engine_config;
