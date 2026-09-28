//! Tolk ABI schemas, type resolution, and runtime TON cell decoding.
//!
//! Load a [`ContractABI`] from compiler ABI JSON with `serde_json`, then call
//! [`unpack_from_slice`] with its runtime type index. No generated Rust bindings,
//! compiler, or debug information are required.

pub mod abi;
pub mod dynamic_unpack;
pub mod types_kernel;

pub use abi::ContractABI;
pub use dynamic_unpack::{UnpackSchema, UnpackedValue, unpack_from_slice};
pub use types_kernel::{Ty, TyIdx, TyResolver};
