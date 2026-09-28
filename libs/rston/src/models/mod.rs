//! Blockchain models.

pub use self::account::*;
pub use self::block::*;
pub use self::config::*;
pub use self::currency::*;
pub use self::global_version::*;
pub use self::message::*;
pub use self::shard::*;
pub use self::transaction::*;
pub use self::vm::*;

pub mod account;
pub mod block;
pub mod config;
pub mod currency;
pub mod global_version;
pub mod message;
pub mod shard;
pub mod transaction;
pub mod vm;

#[cfg(feature = "sync")]
#[doc(hidden)]
mod __checks {
    use super::*;

    assert_impl_all!(crate::cell::Lazy<Message>: Send, Sync);
    assert_impl_all!(Account: Send, Sync);
    assert_impl_all!(Block: Send, Sync);
    assert_impl_all!(Message: Send, Sync);
    assert_impl_all!(Transaction: Send, Sync);
}
