//! Compile-time Rust bindings generated from the JSON ABI emitted by Tolk.
//!
//! Point the [`contract`] attribute at an ABI relative to the consuming crate's
//! `Cargo.toml`:
//!
//! ```ignore
//! #[acton_client::contract(abi = "abi/counter.abi.json")]
//! pub mod counter {}
//!
//! let message = counter::IncreaseCounter {
//!     query_id: 0.into(),
//!     increase_by: 7.into(),
//! };
//! let body = acton_client::encode(&message)?;
//! ```
//!
//! The generated module contains ABI declarations, cell and stack codecs,
//! message senders, getter metadata, and a contract client parameterized by a
//! provider.

extern crate self as acton_client;

pub mod cell;
pub mod deployment;
pub mod dynamic;
pub mod stack;

use std::error::Error as StdError;
use std::fmt;
use std::future::Future;

pub use acton_client_macros::contract;
pub use cell::{
    AbiError, AbiLoad, AbiStore, BitString, CellRef, Dictionary, OwnedSlice, register_custom_codec,
};
pub use deployment::{
    ContractInit, DeployedAddressOptions, ToShard, calculate_deployed_address, decode_code_boc64,
};
pub use dynamic::{
    DynamicAbi, DynamicCallError, DynamicError, DynamicPackFn, DynamicUnpackFn, DynamicValue,
};
pub use num_bigint::BigInt;
pub use stack::StackReader;
pub use tvm_ffi::stack::{Tuple, TupleItem};
pub use tycho_types::cell::Cell;
pub use tycho_types::models::StdAddr;

/// Metadata for a getter exported by a contract ABI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GetMethod {
    pub name: &'static str,
    pub method_id: i32,
}

/// A provider capable of invoking TVM get methods.
pub trait ContractProvider: Sync {
    type Error;

    fn run_get_method(
        &self,
        address: &StdAddr,
        method_id: i32,
        arguments: Tuple,
    ) -> impl Future<Output = Result<Tuple, Self::Error>> + Send;
}

/// Options passed to an internal message send.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SendOptions {
    /// Overrides the provider's default bounce behavior.
    pub bounce: Option<bool>,
}

/// Internal message assembled by a generated contract wrapper.
#[derive(Debug, Clone)]
pub struct InternalMessage {
    pub value: BigInt,
    pub body: Cell,
    pub options: SendOptions,
    pub init: Option<ContractInit>,
}

/// A provider capable of sending an internal message through a sender.
pub trait ContractSender: Sync {
    type Error;
    type Sender: Sync + ?Sized;
    type Output;

    fn send_internal(
        &self,
        via: &Self::Sender,
        address: &StdAddr,
        message: InternalMessage,
    ) -> impl Future<Output = Result<Self::Output, Self::Error>> + Send;
}

/// An error returned by a generated contract client.
#[derive(Debug)]
pub enum ClientError<E> {
    Provider(E),
    Abi(AbiError),
}

impl<E: fmt::Display> fmt::Display for ClientError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Provider(error) => write!(formatter, "contract provider failed: {error}"),
            Self::Abi(error) => write!(formatter, "invalid ABI value: {error}"),
        }
    }
}

impl<E: StdError + 'static> StdError for ClientError<E> {}

impl<E> From<AbiError> for ClientError<E> {
    fn from(error: AbiError) -> Self {
        Self::Abi(error)
    }
}

impl<E> From<tycho_types::error::Error> for ClientError<E> {
    fn from(error: tycho_types::error::Error) -> Self {
        Self::Abi(AbiError::Cell(error))
    }
}

/// Serializes a generated ABI value into an ordinary cell.
pub fn encode<T: AbiStore>(value: &T) -> Result<Cell, AbiError> {
    value.to_cell()
}

/// Deserializes a generated ABI value and rejects trailing bits or references.
pub fn decode<T: AbiLoad>(cell: &Cell) -> Result<T, AbiError> {
    T::from_cell(cell)
}

#[doc(hidden)]
pub mod __private {
    pub use num_bigint;
    pub use tvm_ffi;
    pub use tycho_types;
}
