#![warn(missing_docs)]
#![allow(clippy::question_mark)]

//! TON cells, serialization, blockchain models, and wallets.
//!
//! This crate is a collection of basic structures and models for the
//! TON blockchain. The [`Cell`] represents the core
//! data structure which is used as an atom for building other structures.
//!
//! *Compiler support: [requires `rustc` 1.88+][msrv]*
//!
//! [msrv]: #supported-rust-versions
//!
//! ## `Cell` vs `CellSlice` vs `CellBuilder`
//!
//! - [`Cell`] is an immutable tree and provides only basic methods for accessing
//!   nodes and some meta info.
//!
//! - [`CellSlice`] is a read-only view for a part of some cell. It can only
//!   be obtained from an existing cell. A cell contains **up to 1023 bits** and
//!   **up to 4 references**. Minimal data unit is bit, so a cell slice is similar
//!   to a couple of ranges (bit range and refs range).
//!
//! - [`CellBuilder`] is used to create a new cell. It is used as an append-only
//!   data structure and is the only way to create a new cell with the provided data.
//!   Cell creation depends on a context (e.g. message creation in a wallet or a
//!   TVM execution with gas tracking), so [`CellBuilder::build_ext`] accepts
//!   a [`CellContext`] parameter which can be used to track and modify cells creation.
//!
//! ## BOC
//!
//! BOC (Bag Of Cells) is a format for representing a tree of cells as bytes.
//! [`Boc`] type is used to convert between bytes and **cells** of the same family.
//! [`BocRepr`] helper can be used to convert between bytes and **models** (which
//! are representable as cells).
//!
//! ### Merkle stuff
//!
//! - Pruned branch is a "building block" of merkle structures. A single pruned branch
//!   cell replaces a whole subtree and contains just the hash of its root cell hash.
//!
//! - [`MerkleProof`] contains a subset of original tree of cells. In most cases
//!   it is created from [`UsageTree`] of some visited cells. Merkle proof is used
//!   to proof that something was presented in the origin tree and provide some additional
//!   context.
//!
//! - [`MerkleUpdate`] describes a difference between two trees of cells. It can be
//!   applied to old cell to create a new cell.
//!
//! ### Numeric stuff
//!
//! This crate introduces some unusual number types with custom bit size or variable
//! encoding. They are only used in models, but may be useful in user code.
//!
//! ### Dictionaries
//!
//! Dictionary, erroneously called HashmapE in the original TLB schema, is an
//! important building block of blockchain models. It is similar to `BTreeMap`.
//! Dictionary is an immutable structure over tree of cells with fixed-length
//! keys and arbitrary values. Updates create a new cell tree each time, so
//! it's quite an expensive data structure to work with.
//!
//! ### Models
//!
//! There is a simple definition of nearly all blockchain models. This definition
//! doesn't contain any complex logic, but could be extended via extension traits.
//! The names and structure of the models are slightly different from the
//! definition in the TLB for the sake of consistency of use.
//!
//! All models implement [`Load`] and [`Store`] traits for conversion from/to cells.
//!
//! - [`RawDict`] constrains only key size in bits. It is useful when a dictionary
//!   can contain multiple types of values.
//!
//! - [`Dict`] is a strongly typed version of definition and is a preferable way
//!   of working with this data structure. Key type must implement [`DictKey`] trait,
//!   which is implemented for numbers and addresses.
//!
//! - [`AugDict`] adds additional values for all nodes. You can use it to quickly
//!   access a subtotal of values for each subtree.
//!   NOTE: this type is partially implemented due to its complexity.
//!
//! ## Wallets
//!
//! The [`wallet`] module derives wallet addresses, reads contract storage, and builds
//! signed external messages for V2, V3, V4, and V5R1 wallets.
//! [`wallet::WalletMessage`] pairs an outgoing message with its send-mode flags.
//! The module also provides embedded contract code and code-hash lookup by revision.
//! See the [wallet examples](wallet#derive-a-wallet-address) for address derivation,
//! transfer signing, and storage decoding.
//!
//! Wallet support requires the `wallet` feature, which is enabled by default.
//! The caller fetches account state, supplies the sequence number, and sends the resulting BoC.
//! The [`mnemonic`] module validates TON mnemonic phrases and derives Ed25519 keys.
//!
//! ## Supported Rust Versions
//!
//! This crate is built against the latest stable release. The minimum supported
//! version is 1.88. The current crate version is not guaranteed to build on
//! Rust versions earlier than the minimum supported version.
//!
//! [`Cell`]: cell::Cell
//! [`Rc`]: std::rc::Rc
//! [`Arc`]: std::sync::Arc
//! [`RcCell`]: prelude::RcCell
//! [`ArcCell`]: prelude::ArcCell
//! [`CellSlice`]: cell::CellSlice
//! [`CellBuilder`]: cell::CellBuilder
//! [`Cell::as_slice`]: cell::CellImpl::as_slice
//! [`CellBuilder::build_ext`]: cell::CellBuilder::build_ext
//! [`CellContext`]: cell::CellContext
//! [`Boc`]: boc::Boc
//! [`BocRepr`]: boc::BocRepr
//! [`UsageTree`]: cell::UsageTree
//! [`MerkleProof`]: merkle::MerkleProof
//! [`MerkleUpdate`]: merkle::MerkleUpdate
//! [`RawDict`]: dict::RawDict
//! [`Dict`]: dict::Dict
//! [`DictKey`]: dict::DictKey
//! [`AugDict`]: dict::AugDict
//! [`Load`]: cell::Load
//! [`Store`]: cell::Store
/// Prevents using `From::from` for plain error conversion.
macro_rules! ok {
    ($e:expr $(,)?) => {
        match $e {
            core::result::Result::Ok(val) => val,
            core::result::Result::Err(err) => return core::result::Result::Err(err),
        }
    };
}

#[allow(unused)]
macro_rules! assert_impl_all {
    ($type:ty: $($trait:path),+ $(,)?) => {
        const _: fn() = || {
            // Only callable when `$type` implements all traits in `$($trait)+`.
            fn assert_impl_all<T: ?Sized $(+ $trait)+>() {}
            assert_impl_all::<$type>();
        };
    };
}

extern crate self as rston;

pub mod boc;
pub mod cell;
pub mod crc;
pub mod dict;
pub mod merkle;
pub mod mnemonic;
pub mod num;
pub mod prelude;
pub mod util;
#[cfg(feature = "wallet")]
pub mod wallet;

#[cfg(feature = "models")]
pub mod models;

#[cfg(feature = "arbitrary")]
pub mod arbitrary;

pub mod error;
#[cfg(test)]
mod tests {
    use crate::cell::{CellTreeStats, MAX_BIT_LEN};
    use crate::prelude::*;
    use crate::util::decode_base64;

    #[test]
    fn correct_deserialization() {
        let data = decode_base64("te6ccgEBBAEAzwACg4AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAEAIBAEAAAAAAAAAAAAAAAAAAAAAAAAAAm2c6ClpzoTVSAHvzVQGDAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAHKq1w7OAAkYAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAACRwAwBAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAEljGP8=").unwrap();

        let cell = Boc::decode(&data).unwrap();
        assert_eq!(
            cell.repr_hash(),
            &[
                0x63, 0xd4, 0x75, 0x13, 0x9a, 0xc1, 0x4f, 0x3e, 0xfe, 0x69, 0x0e, 0xd7, 0xfd, 0x4f,
                0xf0, 0x02, 0x1c, 0xf2, 0x6b, 0xc4, 0xab, 0xd0, 0xaf, 0x01, 0x40, 0xa3, 0xb4, 0xc8,
                0x95, 0xf0, 0x73, 0x76
            ]
        );

        let serialized = Boc::encode(cell.as_ref());
        assert_eq!(serialized, data);
    }

    #[test]
    fn big_cell_deserialization() {
        let data = decode_base64("te6ccgIDAAwAAQAAAACIAAAEBAABAAEAAQABAAEEBAACAAIAAgACAAIEBAADAAMAAwADAAMEBAAEAAQABAAEAAQEBAAFAAUABQAFAAUEBAAGAAYABgAGAAYEBAAHAAcABwAHAAcEBAAIAAgACAAIAAgEBAAJAAkACQAJAAkEBAAKAAoACgAKAAoEBAALAAsACwALAAsABAAA").unwrap();
        let cell = Boc::decode(data).unwrap();

        let stats = cell.compute_unique_stats(1 << 22).unwrap();
        assert_eq!(
            stats,
            CellTreeStats {
                bit_count: 192,
                cell_count: 12
            }
        );
    }

    #[test]
    fn test_builder() -> anyhow::Result<()> {
        let parsed_cell = Boc::decode_base64("te6ccgEBAQEAAwAAAbE=")?;

        let mut builder = CellBuilder::new();
        builder.store_bit_one()?;
        builder.store_bit_zero()?;
        builder.store_bit_one()?;
        builder.store_bit_one()?;
        builder.store_bit_zero()?;
        builder.store_bit_zero()?;
        builder.store_bit_zero()?;
        let built_cell = builder.build()?;

        assert_eq!(parsed_cell.repr_hash(), built_cell.repr_hash());

        let parsed_cell = Boc::decode_base64(
            "te6ccgEBAQEAggAA////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////",
        )?;

        let mut builder = CellBuilder::new();
        for _ in 0..MAX_BIT_LEN {
            builder.store_bit_one()?;
        }
        assert!(builder.store_bit_one().is_err());
        let built_cell = builder.build()?;

        assert_eq!(parsed_cell.repr_hash(), built_cell.repr_hash());

        let mut builder = CellBuilder::new();
        builder.store_bit_one()?;
        builder.store_u128(0xaaffaaffaaffaaffaaffaaffaaffaaff)?;
        let cell = builder.build()?;

        let mut builder = CellBuilder::new();
        builder.store_bit_one()?;
        builder.store_u64(0xaaffaaffaaffaaff)?;
        builder.store_u64(0xaaffaaffaaffaaff)?;
        assert_eq!(cell.as_ref(), builder.build()?.as_ref());

        let mut builder = CellBuilder::new();
        builder.store_zeros(1020)?;
        builder.store_small_uint(0x5, 3)?;
        builder.build()?;

        let mut builder = CellBuilder::new();
        builder.store_small_uint(5, 3)?;
        builder.store_u256(HashBytes::wrap(&[
            0xdf, 0x86, 0xce, 0xbc, 0xe8, 0xd5, 0xab, 0x0c, 0x69, 0xb4, 0xce, 0x33, 0xfe, 0x9b,
            0x0e, 0x2c, 0xdf, 0x69, 0xa3, 0xe1, 0x13, 0x7e, 0x64, 0x85, 0x6b, 0xbc, 0xfd, 0x39,
            0xe7, 0x9b, 0xc1, 0x6f,
        ]))?;
        let cell = builder.build()?;

        let target_cell =
            Boc::decode_base64("te6ccgEBAQEAIwAAQbvw2dedGrVhjTaZxn/TYcWb7TR8Im/MkK13n6c883gt8A==")?;
        assert_eq!(cell.as_ref(), target_cell.as_ref());

        let mut builder = CellBuilder::new();
        builder.store_zeros(3)?;
        builder.store_raw(&[0xdd, 0x55], 10)?;
        builder.store_reference(target_cell)?;
        builder.store_reference(cell)?;
        let cell = builder.build()?;

        let mut builder = CellBuilder::new();
        builder.store_slice(cell.as_slice()?)?;
        let cell = builder.build()?;
        println!("{}", cell.display_tree());

        Ok(())
    }

    #[test]
    fn test_tx() {
        let cell = Boc::decode_base64("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").unwrap();

        let stats = cell.compute_unique_stats(1 << 22).unwrap();
        assert_eq!(
            stats,
            CellTreeStats {
                bit_count: 4681,
                cell_count: 266
            }
        );
    }
}
