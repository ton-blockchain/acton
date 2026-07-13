use crate::common::{run_get_method, run_get_method_with_stack};
use num_bigint::BigInt;
use sha2::{Digest, Sha256};
use tvm_ffi::stack::{Tuple, TupleItem};
use tycho_types::cell::{Cell, CellBuilder, CellDataBuilder, HashBytes, Load};
use tycho_types::dict::{Dict, DictKey, LoadDictKey};
use tycho_types::models::IntAddr;

#[derive(Debug, Clone)]
pub struct DnsData {
    pub domain: String,
    pub next_resolver: Option<IntAddr>,
    pub wallet: Option<IntAddr>,
    pub site_adnl: Option<HashBytes>,
    pub storage_bag_id: Option<HashBytes>,
}

#[derive(tvm_ffi::FromStackTuple)]
struct SingleCell {
    value: Cell,
}

#[derive(tvm_ffi::FromStackTuple)]
struct DnsResolveResult {
    resolved_bits: BigInt,
    records: Option<Cell>,
}

#[must_use]
pub fn get_dns_data(
    address: String,
    code: Cell,
    data: Cell,
    libs: Option<&str>,
) -> Option<DnsData> {
    get_dns_data_result(address, code, data, libs).ok()
}

fn get_dns_data_result(
    address: String,
    code: Cell,
    data: Cell,
    libs: Option<&str>,
) -> anyhow::Result<DnsData> {
    let domain = get_domain(address.clone(), code.clone(), data.clone(), libs)?;
    let mut name = CellBuilder::new();
    name.store_u8(0)?;
    let stack = Tuple(vec![
        TupleItem::Slice(name.build()?),
        TupleItem::Int(BigInt::from(0)),
    ]);
    let resolved: DnsResolveResult =
        run_get_method_with_stack(address, code, data, libs, "dnsresolve", stack)?;
    if resolved.resolved_bits != BigInt::from(8) {
        anyhow::bail!("dnsresolve resolved an unexpected number of bits");
    }

    let mut result = DnsData {
        domain,
        next_resolver: None,
        wallet: None,
        site_adnl: None,
        storage_bag_id: None,
    };
    let Some(root) = resolved.records else {
        return Ok(result);
    };
    let records = Dict::<HashBytes, Cell>::from_raw(Some(root));
    if let Some(cell) = lookup_dns_record(&records, "wallet")? {
        result.wallet = parse_dns_address_record(&cell, 0x9fd3)?;
    }
    if let Some(cell) = lookup_dns_record(&records, "dns_next_resolver")? {
        result.next_resolver = parse_dns_address_record(&cell, 0xba93)?;
    }
    if let Some(cell) = lookup_dns_record(&records, "site")? {
        result.site_adnl = parse_dns_hash_record(&cell, 0xad01)?;
    }
    if let Some(cell) = lookup_dns_record(&records, "storage")? {
        result.storage_bag_id = parse_dns_hash_record(&cell, 0x7473)?;
    }
    Ok(result)
}

fn get_domain(
    address: String,
    code: Cell,
    data: Cell,
    libs: Option<&str>,
) -> anyhow::Result<String> {
    if let Ok(value) = run_get_method::<SingleCell>(
        address.clone(),
        code.clone(),
        data.clone(),
        libs,
        "get_domain",
    ) {
        let bytes = cell_bytes(&value.value)?;
        return Ok(format!("{}.ton", String::from_utf8(bytes)?));
    }

    let value: SingleCell = run_get_method(address, code, data, libs, "get_full_domain")?;
    let bytes = cell_bytes(&value.value)?;
    let mut labels = bytes
        .split(|byte| *byte == 0)
        .filter(|label| !label.is_empty())
        .map(|label| String::from_utf8(label.to_vec()))
        .collect::<Result<Vec<_>, _>>()?;
    labels.reverse();
    Ok(labels.join("."))
}

fn cell_bytes(cell: &Cell) -> anyhow::Result<Vec<u8>> {
    let mut slice = cell.as_slice_allow_exotic();
    let bit_len = slice.size_bits();
    if !bit_len.is_multiple_of(8) {
        anyhow::bail!("DNS name is not byte-aligned");
    }
    let mut bytes = vec![0; usize::from(bit_len / 8)];
    slice.load_raw(&mut bytes, bit_len)?;
    Ok(bytes)
}

fn lookup_dns_record(
    records: &Dict<HashBytes, Cell>,
    category: &str,
) -> anyhow::Result<Option<Cell>> {
    let key = HashBytes(Sha256::digest(category.as_bytes()).into());
    records.get(key).map_err(Into::into)
}

fn parse_dns_address_record(cell: &Cell, expected_tag: u16) -> anyhow::Result<Option<IntAddr>> {
    let mut slice = cell.as_slice_allow_exotic();
    if slice.load_u16()? != expected_tag {
        anyhow::bail!("unexpected DNS address record tag");
    }
    Ok(Some(IntAddr::load_from(&mut slice)?))
}

fn parse_dns_hash_record(cell: &Cell, expected_tag: u16) -> anyhow::Result<Option<HashBytes>> {
    let mut slice = cell.as_slice_allow_exotic();
    if slice.load_u16()? != expected_tag {
        anyhow::bail!("unexpected DNS hash record tag");
    }
    Ok(Some(slice.load_u256()?))
}

#[derive(Debug, Clone, tvm_ffi::FromStackTuple)]
pub struct FixedPriceSaleData {
    pub magic: BigInt,
    pub is_complete: bool,
    pub created_at: BigInt,
    pub marketplace_address: IntAddr,
    pub nft_address: IntAddr,
    pub nft_owner_address: Option<IntAddr>,
    pub full_price: BigInt,
    pub marketplace_fee_address: IntAddr,
    pub marketplace_fee: BigInt,
    pub royalty_address: IntAddr,
    pub royalty_amount: BigInt,
}

#[must_use]
pub fn get_fixed_price_sale_data(
    address: String,
    code: Cell,
    data: Cell,
    libs: Option<&str>,
) -> Option<FixedPriceSaleData> {
    let result: FixedPriceSaleData =
        run_get_method(address, code, data, libs, "get_sale_data").ok()?;
    (result.magic == BigInt::from(0x4649_5850u32)).then_some(result)
}

#[derive(Debug, Clone, tvm_ffi::FromStackTuple)]
pub struct FixedPriceSaleV4Data {
    pub is_complete: bool,
    pub created_at: BigInt,
    pub marketplace_address: IntAddr,
    pub nft_address: IntAddr,
    pub nft_owner_address: Option<IntAddr>,
    pub full_price: BigInt,
    pub marketplace_fee_address: IntAddr,
    pub marketplace_fee: BigInt,
    pub royalty_address: IntAddr,
    pub royalty_amount: BigInt,
    pub sold_at: BigInt,
    pub sold_query_id: BigInt,
    pub jetton_prices: Option<Cell>,
}

#[must_use]
pub fn get_fixed_price_sale_v4_data(
    address: String,
    code: Cell,
    data: Cell,
    libs: Option<&str>,
) -> Option<FixedPriceSaleV4Data> {
    run_get_method(address, code, data, libs, "get_fix_price_data_v4").ok()
}

#[derive(Debug, Clone)]
pub struct AuctionData {
    pub end: bool,
    pub end_time: BigInt,
    pub marketplace_address: IntAddr,
    pub nft_address: IntAddr,
    pub nft_owner_address: Option<IntAddr>,
    pub last_bid: BigInt,
    pub last_member: Option<IntAddr>,
    pub min_step: BigInt,
    pub marketplace_fee_address: IntAddr,
    pub marketplace_fee_factor: BigInt,
    pub marketplace_fee_base: BigInt,
    pub royalty_fee_address: IntAddr,
    pub royalty_fee_factor: BigInt,
    pub royalty_fee_base: BigInt,
    pub max_bid: BigInt,
    pub min_bid: BigInt,
    pub created_at: BigInt,
    pub last_bid_at: BigInt,
    pub is_canceled: bool,
    pub activated: Option<bool>,
    pub step_time: Option<BigInt>,
    pub last_query_id: Option<BigInt>,
    pub jetton_wallet: Option<IntAddr>,
    pub jetton_master: Option<IntAddr>,
    pub is_broken_state: Option<bool>,
    pub public_key: Option<BigInt>,
}

#[derive(tvm_ffi::FromStackTuple)]
struct AuctionV3Data {
    magic: BigInt,
    end: bool,
    end_time: BigInt,
    marketplace_address: IntAddr,
    nft_address: IntAddr,
    nft_owner_address: Option<IntAddr>,
    last_bid: BigInt,
    last_member: Option<IntAddr>,
    min_step: BigInt,
    marketplace_fee_address: IntAddr,
    marketplace_fee_factor: BigInt,
    marketplace_fee_base: BigInt,
    royalty_fee_address: IntAddr,
    royalty_fee_factor: BigInt,
    royalty_fee_base: BigInt,
    max_bid: BigInt,
    min_bid: BigInt,
    created_at: BigInt,
    last_bid_at: BigInt,
    is_canceled: bool,
}

#[derive(tvm_ffi::FromStackTuple)]
struct AuctionV4Data {
    activated: bool,
    end: bool,
    end_time: BigInt,
    marketplace_address: IntAddr,
    nft_address: IntAddr,
    nft_owner_address: Option<IntAddr>,
    last_bid: BigInt,
    last_member: Option<IntAddr>,
    min_step: BigInt,
    marketplace_fee_address: IntAddr,
    marketplace_fee_factor: BigInt,
    marketplace_fee_base: BigInt,
    royalty_fee_address: IntAddr,
    royalty_fee_factor: BigInt,
    royalty_fee_base: BigInt,
    max_bid: BigInt,
    min_bid: BigInt,
    created_at: BigInt,
    last_bid_at: BigInt,
    is_canceled: bool,
    step_time: BigInt,
    last_query_id: BigInt,
    jetton_wallet: Option<IntAddr>,
    jetton_master: Option<IntAddr>,
    is_broken_state: bool,
    public_key: BigInt,
}

#[must_use]
pub fn get_auction_data(
    address: String,
    code: Cell,
    data: Cell,
    libs: Option<&str>,
) -> Option<AuctionData> {
    if let Ok(result) = run_get_method::<AuctionV4Data>(
        address.clone(),
        code.clone(),
        data.clone(),
        libs,
        "get_auction_data_v4",
    ) {
        return Some(AuctionData {
            end: result.end,
            end_time: result.end_time,
            marketplace_address: result.marketplace_address,
            nft_address: result.nft_address,
            nft_owner_address: result.nft_owner_address,
            last_bid: result.last_bid,
            last_member: result.last_member,
            min_step: result.min_step,
            marketplace_fee_address: result.marketplace_fee_address,
            marketplace_fee_factor: result.marketplace_fee_factor,
            marketplace_fee_base: result.marketplace_fee_base,
            royalty_fee_address: result.royalty_fee_address,
            royalty_fee_factor: result.royalty_fee_factor,
            royalty_fee_base: result.royalty_fee_base,
            max_bid: result.max_bid,
            min_bid: result.min_bid,
            created_at: result.created_at,
            last_bid_at: result.last_bid_at,
            is_canceled: result.is_canceled,
            activated: Some(result.activated),
            step_time: Some(result.step_time),
            last_query_id: Some(result.last_query_id),
            jetton_wallet: result.jetton_wallet,
            jetton_master: result.jetton_master,
            is_broken_state: Some(result.is_broken_state),
            public_key: Some(result.public_key),
        });
    }

    let result: AuctionV3Data = run_get_method(address, code, data, libs, "get_sale_data").ok()?;
    (result.magic == BigInt::from(0x41_55_43_u32)).then_some(AuctionData {
        end: result.end,
        end_time: result.end_time,
        marketplace_address: result.marketplace_address,
        nft_address: result.nft_address,
        nft_owner_address: result.nft_owner_address,
        last_bid: result.last_bid,
        last_member: result.last_member,
        min_step: result.min_step,
        marketplace_fee_address: result.marketplace_fee_address,
        marketplace_fee_factor: result.marketplace_fee_factor,
        marketplace_fee_base: result.marketplace_fee_base,
        royalty_fee_address: result.royalty_fee_address,
        royalty_fee_factor: result.royalty_fee_factor,
        royalty_fee_base: result.royalty_fee_base,
        max_bid: result.max_bid,
        min_bid: result.min_bid,
        created_at: result.created_at,
        last_bid_at: result.last_bid_at,
        is_canceled: result.is_canceled,
        activated: None,
        step_time: None,
        last_query_id: None,
        jetton_wallet: None,
        jetton_master: None,
        is_broken_state: None,
        public_key: None,
    })
}

#[derive(Debug, Clone)]
pub struct TelemintData {
    pub token_name: String,
    pub bidder_address: Option<IntAddr>,
    pub bid: BigInt,
    pub bid_ts: BigInt,
    pub min_bid: BigInt,
    pub end_time: BigInt,
    pub beneficiary_address: Option<IntAddr>,
    pub initial_min_bid: BigInt,
    pub max_bid: BigInt,
    pub min_bid_step: BigInt,
    pub min_extend_time: BigInt,
    pub duration: BigInt,
    pub royalty_numerator: BigInt,
    pub royalty_denominator: BigInt,
    pub royalty_destination: IntAddr,
}

#[derive(tvm_ffi::FromStackTuple)]
struct TelemintAuctionState {
    bidder_address: Option<IntAddr>,
    bid: BigInt,
    bid_ts: BigInt,
    min_bid: BigInt,
    end_time: BigInt,
}

#[derive(tvm_ffi::FromStackTuple)]
struct TelemintAuctionConfig {
    beneficiary_address: Option<IntAddr>,
    initial_min_bid: BigInt,
    max_bid: BigInt,
    min_bid_step: BigInt,
    min_extend_time: BigInt,
    duration: BigInt,
}

#[derive(tvm_ffi::FromStackTuple)]
struct RoyaltyParams {
    numerator: BigInt,
    denominator: BigInt,
    destination: IntAddr,
}

#[must_use]
pub fn get_telemint_data(
    address: String,
    code: Cell,
    data: Cell,
    libs: Option<&str>,
) -> Option<TelemintData> {
    let name: SingleCell = run_get_method(
        address.clone(),
        code.clone(),
        data.clone(),
        libs,
        "get_telemint_token_name",
    )
    .ok()?;
    let token_name = String::from_utf8(snake_bytes(name.value).ok()?).ok()?;
    let state = run_get_method::<TelemintAuctionState>(
        address.clone(),
        code.clone(),
        data.clone(),
        libs,
        "get_telemint_auction_state",
    )
    .ok();
    let config: TelemintAuctionConfig = run_get_method(
        address.clone(),
        code.clone(),
        data.clone(),
        libs,
        "get_telemint_auction_config",
    )
    .ok()?;
    let royalty: RoyaltyParams =
        run_get_method(address, code, data, libs, "royalty_params").ok()?;

    Some(TelemintData {
        token_name,
        bidder_address: state
            .as_ref()
            .and_then(|state| state.bidder_address.clone()),
        bid: state
            .as_ref()
            .map(|state| state.bid.clone())
            .unwrap_or_default(),
        bid_ts: state
            .as_ref()
            .map(|state| state.bid_ts.clone())
            .unwrap_or_default(),
        min_bid: state
            .as_ref()
            .map(|state| state.min_bid.clone())
            .unwrap_or_default(),
        end_time: state
            .as_ref()
            .map(|state| state.end_time.clone())
            .unwrap_or_default(),
        beneficiary_address: config.beneficiary_address,
        initial_min_bid: config.initial_min_bid,
        max_bid: config.max_bid,
        min_bid_step: config.min_bid_step,
        min_extend_time: config.min_extend_time,
        duration: config.duration,
        royalty_numerator: royalty.numerator,
        royalty_denominator: royalty.denominator,
        royalty_destination: royalty.destination,
    })
}

fn snake_bytes(mut cell: Cell) -> anyhow::Result<Vec<u8>> {
    let mut result = Vec::new();
    loop {
        let mut slice = cell.as_slice()?;
        let bit_len = slice.size_bits();
        if !bit_len.is_multiple_of(8) {
            anyhow::bail!("snake string is not byte-aligned");
        }
        let mut part = vec![0; usize::from(bit_len / 8)];
        slice.load_raw(&mut part, bit_len)?;
        result.extend(part);
        match slice.size_refs() {
            0 => return Ok(result),
            1 => cell = slice.load_reference_cloned()?,
            _ => anyhow::bail!("snake string cell has multiple references"),
        }
    }
}

#[derive(Debug, Clone, tvm_ffi::FromStackTuple)]
pub struct VestingData {
    pub start_time: BigInt,
    pub total_duration: BigInt,
    pub unlock_period: BigInt,
    pub cliff_duration: BigInt,
    pub total_amount: BigInt,
    pub sender_address: IntAddr,
    pub owner_address: IntAddr,
    pub whitelist: Option<Cell>,
}

#[must_use]
pub fn get_vesting_data(
    address: String,
    code: Cell,
    data: Cell,
    libs: Option<&str>,
) -> Option<VestingData> {
    run_get_method(address, code, data, libs, "get_vesting_data").ok()
}

pub fn parse_vesting_whitelist(cell: Option<&Cell>) -> anyhow::Result<Vec<IntAddr>> {
    #[derive(Clone, Eq, Ord, PartialEq, PartialOrd)]
    struct Bits264([u8; 33]);

    impl DictKey for Bits264 {
        const BITS: u16 = 264;
    }

    impl LoadDictKey for Bits264 {
        fn load_from_data(data: &CellDataBuilder) -> Option<Self> {
            Some(Self(data.raw_data()[..33].try_into().ok()?))
        }
    }

    let dict = Dict::<Bits264, ()>::from_raw(cell.cloned());
    let mut result = Vec::new();
    for entry in dict.iter() {
        let (key, ()) = entry?;
        let workchain = key.0[0] as i8;
        result.push(IntAddr::Std(tycho_types::models::StdAddr::new(
            workchain,
            HashBytes(key.0[1..].try_into().expect("fixed address size")),
        )));
    }
    Ok(result)
}
