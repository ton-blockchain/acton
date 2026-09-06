//! Messages for Acton's own Jetton and NFT templates. Bundled code is taken from
//! their generated wrappers, keeping traffic independent of external metadata hosts.

use super::metadata::Asset;
use anyhow::Result;
use tycho_types::{
    boc::Boc,
    cell::{Cell, CellBuilder, Store},
    models::{AnyAddr, StateInit, StdAddr},
    num::Tokens,
};

pub(super) const GRAM: u128 = 1_000_000_000;

pub(super) fn build(value: impl Store) -> Result<Cell> {
    Ok(CellBuilder::build_from(value)?)
}

pub(super) fn comment(text: &str) -> Result<Cell> {
    let mut builder = CellBuilder::new();
    builder.store_u32(0)?;
    builder.store_raw(text.as_bytes(), (text.len() * 8) as u16)?;
    Ok(builder.build()?)
}

pub(super) fn jetton(owner: &StdAddr) -> Result<(StdAddr, StateInit)> {
    let data = build((Tokens::ZERO, owner, AnyAddr::None, Asset::jetton().cell()?))?;
    deployment(include_bytes!("contracts/JettonMinter.boc"), data)
}

pub(super) fn collection(owner: &StdAddr) -> Result<(StdAddr, StateInit)> {
    let content = build((Asset::collection().cell()?, Cell::default()))?;
    let data = build((
        owner,
        0u64,
        content,
        Boc::decode(include_bytes!("contracts/NftItem.boc"))?,
        build((0u16, 1000u16, owner))?,
    ))?;
    deployment(include_bytes!("contracts/NftCollection.boc"), data)
}

fn deployment(code: &[u8], data: Cell) -> Result<(StdAddr, StateInit)> {
    let init = StateInit {
        code: Some(Boc::decode(code)?),
        data: Some(data),
        ..Default::default()
    };
    let address = StdAddr::new(0, *build(&init)?.repr_hash());
    Ok((address, init))
}

pub(super) fn mint_jettons(owner: &StdAddr) -> Result<Cell> {
    let internal = build((
        (0x178d4519u32, 0u64),
        Tokens::new(1000 * GRAM),
        AnyAddr::None,
        owner,
        Tokens::ZERO,
        false,
    ))?;
    build((0x642b7d07u32, 0u64, owner, Tokens::new(GRAM), internal))
}

pub(super) fn transfer_jettons(owner: &StdAddr, recipient: &StdAddr) -> Result<Cell> {
    build((
        (0x0f8a7ea5u32, 1u64),
        Tokens::new(100 * GRAM),
        recipient,
        owner,
        None::<Cell>,
        (Tokens::new(10_000_000), false),
    ))
}

pub(super) fn burn_jettons(owner: &StdAddr) -> Result<Cell> {
    build((
        0x595f07bcu32,
        2u64,
        Tokens::new(50 * GRAM),
        owner,
        None::<Cell>,
    ))
}

pub(super) fn mint_nft(owner: &StdAddr) -> Result<Cell> {
    build((
        1u32,
        0u64,
        0u64,
        Tokens::new(GRAM),
        build((owner, Asset::item().cell()?))?,
    ))
}

pub(super) fn transfer_nft(owner: &StdAddr, recipient: &StdAddr) -> Result<Cell> {
    build((
        (0x5fcc3d14u32, 1u64),
        recipient,
        owner,
        None::<Cell>,
        Tokens::new(10_000_000),
        false,
    ))
}
