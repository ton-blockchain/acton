//! LiteAPI protocol and server transport code for local TON liteserver support.
//!
//! This crate vendors the LiteAPI, TL, and ADNL protocol layers from
//! `tonutils` v1.1.0 by Nikita Ugnich, preserving the original MIT license in
//! `LICENSE.tonutils`. The copied modules intentionally keep the original
//! request/response surface so a local liteserver can grow toward broad
//! LiteAPI coverage instead of only the methods currently needed by one client.

#![allow(warnings)]
#![allow(clippy::all)]

pub mod adnl;
pub mod liteclient;
pub mod tl;
