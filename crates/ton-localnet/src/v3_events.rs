use crate::localnet::LocalnetTransaction;
use crate::storage::{JettonWalletMeta, NftItemMeta};
use crate::types::{Addr, BocBytes, Hash256};
use tycho_types::boc::Boc;
use tycho_types::cell::{Cell, CellBuilder, CellSlice, Load};
use tycho_types::models::{AnyAddr, IntAddr};

const JETTON_TRANSFER_OPCODE: u32 = 0x0f8a_7ea5;
const JETTON_BURN_OPCODE: u32 = 0x595f_07bc;
const NFT_TRANSFER_OPCODE: u32 = 0x5fcc_3d14;

#[derive(Clone, Debug)]
pub struct JettonTransferEvent {
    pub query_id: String,
    pub source: Addr,
    pub destination: Addr,
    pub amount: String,
    pub source_wallet: Addr,
    pub jetton_master: Addr,
    pub transaction_hash: Hash256,
    pub transaction_lt: u64,
    pub transaction_now: u32,
    pub transaction_aborted: bool,
    pub response_destination: Option<Addr>,
    pub custom_payload: Option<BocBytes>,
    pub forward_ton_amount: String,
    pub forward_payload: Option<BocBytes>,
}

#[derive(Clone, Debug)]
pub struct JettonBurnEvent {
    pub query_id: String,
    pub owner: Addr,
    pub jetton_wallet: Addr,
    pub jetton_master: Addr,
    pub transaction_hash: Hash256,
    pub transaction_lt: u64,
    pub transaction_now: u32,
    pub transaction_aborted: bool,
    pub amount: String,
    pub response_destination: Option<Addr>,
    pub custom_payload: Option<BocBytes>,
}

#[derive(Clone, Debug)]
pub struct NftTransferEvent {
    pub query_id: String,
    pub nft_address: Addr,
    pub nft_collection: Addr,
    pub transaction_hash: Hash256,
    pub transaction_lt: u64,
    pub transaction_now: u32,
    pub transaction_aborted: bool,
    pub old_owner: Addr,
    pub new_owner: Addr,
    pub response_destination: Option<Addr>,
    pub custom_payload: Option<BocBytes>,
    pub forward_amount: String,
    pub forward_payload: Option<BocBytes>,
}

pub(crate) fn parse_jetton_transfer(
    transaction: &LocalnetTransaction,
    wallet: &JettonWalletMeta,
) -> anyhow::Result<Option<JettonTransferEvent>> {
    if transaction.in_msg.opcode != Some(JETTON_TRANSFER_OPCODE) {
        return Ok(None);
    }
    let body_cell = message_body(&transaction.in_msg.body)?;
    let mut body = body_cell.as_slice_allow_exotic();
    if body.load_u32()? != JETTON_TRANSFER_OPCODE {
        return Ok(None);
    }
    let query_id = body.load_u64()?.to_string();
    let amount = body.load_var_bigint(4, false)?.to_string();
    let Some(destination) = load_optional_address(&mut body)? else {
        return Ok(None);
    };
    let response_destination = load_optional_address(&mut body)?;
    let custom_payload = load_optional_ref(&mut body)?.map(cell_boc);
    let forward_ton_amount = body.load_var_bigint(4, false)?.to_string();
    let forward_payload = load_either_payload(&mut body)?.map(cell_boc);

    Ok(Some(JettonTransferEvent {
        query_id,
        source: wallet.owner_address,
        destination,
        amount,
        source_wallet: wallet.address,
        jetton_master: wallet.jetton_address,
        transaction_hash: transaction.hash,
        transaction_lt: transaction.transaction_id.lt,
        transaction_now: transaction.utime,
        transaction_aborted: transaction.aborted,
        response_destination,
        custom_payload,
        forward_ton_amount,
        forward_payload,
    }))
}

pub(crate) fn parse_jetton_burn(
    transaction: &LocalnetTransaction,
    wallet: &JettonWalletMeta,
) -> anyhow::Result<Option<JettonBurnEvent>> {
    if transaction.in_msg.opcode != Some(JETTON_BURN_OPCODE) {
        return Ok(None);
    }
    let body_cell = message_body(&transaction.in_msg.body)?;
    let mut body = body_cell.as_slice_allow_exotic();
    if body.load_u32()? != JETTON_BURN_OPCODE {
        return Ok(None);
    }
    let query_id = body.load_u64()?.to_string();
    let amount = body.load_var_bigint(4, false)?.to_string();
    let response_destination = load_optional_address(&mut body)?;
    let custom_payload = load_optional_ref(&mut body)?.map(cell_boc);

    Ok(Some(JettonBurnEvent {
        query_id,
        owner: wallet.owner_address,
        jetton_wallet: wallet.address,
        jetton_master: wallet.jetton_address,
        transaction_hash: transaction.hash,
        transaction_lt: transaction.transaction_id.lt,
        transaction_now: transaction.utime,
        transaction_aborted: transaction.aborted,
        amount,
        response_destination,
        custom_payload,
    }))
}

pub(crate) fn parse_nft_transfer(
    transaction: &LocalnetTransaction,
    item: &NftItemMeta,
) -> anyhow::Result<Option<NftTransferEvent>> {
    if transaction.in_msg.opcode != Some(NFT_TRANSFER_OPCODE) {
        return Ok(None);
    }
    let (Some(old_owner), Some(nft_collection)) =
        (transaction.in_msg.source, item.collection_address)
    else {
        return Ok(None);
    };
    let body_cell = message_body(&transaction.in_msg.body)?;
    let mut body = body_cell.as_slice_allow_exotic();
    if body.load_u32()? != NFT_TRANSFER_OPCODE {
        return Ok(None);
    }
    let query_id = body.load_u64()?.to_string();
    let Some(new_owner) = load_optional_address(&mut body)? else {
        return Ok(None);
    };
    let response_destination = load_optional_address(&mut body)?;
    let custom_payload = load_optional_ref(&mut body)?.map(cell_boc);
    let forward_amount = body.load_var_bigint(4, false)?.to_string();
    let forward_payload = load_either_payload(&mut body)?.map(cell_boc);

    Ok(Some(NftTransferEvent {
        query_id,
        nft_address: item.address,
        nft_collection,
        transaction_hash: transaction.hash,
        transaction_lt: transaction.transaction_id.lt,
        transaction_now: transaction.utime,
        transaction_aborted: transaction.aborted,
        old_owner,
        new_owner,
        response_destination,
        custom_payload,
        forward_amount,
        forward_payload,
    }))
}

fn message_body(body: &BocBytes) -> anyhow::Result<Cell> {
    Boc::decode(body).map_err(Into::into)
}

fn load_optional_address(slice: &mut CellSlice<'_>) -> anyhow::Result<Option<Addr>> {
    Ok(match AnyAddr::load_from(slice)? {
        AnyAddr::None => None,
        AnyAddr::Std(address) => Some(Addr::from(&IntAddr::Std(address))),
        AnyAddr::Var(address) => Some(Addr::from(&IntAddr::Var(address))),
        AnyAddr::Ext(_) => anyhow::bail!("external address is not valid in token message body"),
    })
}

fn load_optional_ref(slice: &mut CellSlice<'_>) -> anyhow::Result<Option<Cell>> {
    if slice.load_bit()? {
        Ok(Some(slice.load_reference_cloned()?))
    } else {
        Ok(None)
    }
}

fn load_either_payload(slice: &mut CellSlice<'_>) -> anyhow::Result<Option<Cell>> {
    if slice.load_bit()? {
        return Ok(Some(slice.load_reference_cloned()?));
    }
    if slice.is_empty() {
        return Ok(None);
    }
    Ok(Some(CellBuilder::build_from(*slice)?))
}

fn cell_boc(cell: Cell) -> BocBytes {
    Boc::encode(cell).into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::localnet::{LocalnetMessage, LocalnetTransactionId};
    use num_bigint::BigInt;
    use serde_json::Value;
    use tycho_types::cell::{CellFamily, Store};

    fn addr(byte: u8) -> Addr {
        Addr {
            workchain: 0,
            addr: [byte; 32],
        }
    }

    fn store_address(builder: &mut CellBuilder, address: Option<Addr>) {
        let address = address
            .map(IntAddr::from)
            .map_or(AnyAddr::None, |address| match address {
                IntAddr::Std(address) => AnyAddr::Std(address),
                IntAddr::Var(address) => AnyAddr::Var(address),
            });
        address.store_into(builder, Cell::empty_context()).unwrap();
    }

    fn transaction(opcode: u32, account: Addr, source: Addr, body: Cell) -> LocalnetTransaction {
        let hash = Hash256([9; 32]);
        LocalnetTransaction {
            hash,
            address: account,
            mc_block_seqno: 1,
            utime: 123,
            data: BocBytes::default(),
            aborted: false,
            exit_code: 0,
            transaction_id: LocalnetTransactionId { lt: 456, hash },
            in_msg: LocalnetMessage {
                hash: Hash256([8; 32]),
                hash_norm: None,
                source: Some(source),
                destination: Some(account),
                bounce: true,
                bounced: false,
                value: 1,
                body_hash: Hash256::from(body.repr_hash()),
                body: Boc::encode(body).into(),
                init_state: BocBytes::default(),
                opcode: Some(opcode),
                fwd_fee: 0,
                ihr_fee: 0,
                created_lt: 455,
                extra_currencies: Vec::new(),
            },
            out_msgs: Vec::new(),
            total_fees: 0,
            storage_fees: 0,
            other_fees: 0,
        }
    }

    fn wallet(address: Addr, owner: Addr, jetton: Addr) -> JettonWalletMeta {
        JettonWalletMeta {
            address,
            balance: 100,
            code_hash: Hash256([1; 32]),
            data_hash: Hash256([2; 32]),
            jetton_address: jetton,
            jetton_wallet_code_hash: Hash256([3; 32]),
            last_transaction_lt: 456,
            mintless_is_claimed: None,
            owner_address: owner,
        }
    }

    #[test]
    fn parses_jetton_transfer_body() {
        let wallet_address = addr(1);
        let owner = addr(2);
        let destination = addr(3);
        let response_destination = addr(4);
        let mut body = CellBuilder::new();
        body.store_u32(JETTON_TRANSFER_OPCODE).unwrap();
        body.store_u64(7).unwrap();
        body.store_var_bigint(&BigInt::from(42), 4, false).unwrap();
        store_address(&mut body, Some(destination));
        store_address(&mut body, Some(response_destination));
        body.store_bit_zero().unwrap();
        body.store_var_bigint(&BigInt::from(5), 4, false).unwrap();
        body.store_bit_zero().unwrap();
        let transaction = transaction(
            JETTON_TRANSFER_OPCODE,
            wallet_address,
            owner,
            body.build().unwrap(),
        );

        let event = parse_jetton_transfer(&transaction, &wallet(wallet_address, owner, addr(5)))
            .unwrap()
            .unwrap();
        assert_eq!(event.query_id, "7");
        assert_eq!(event.amount, "42");
        assert_eq!(event.destination, destination);
        assert_eq!(event.response_destination, Some(response_destination));
        assert_eq!(event.forward_ton_amount, "5");
    }

    #[test]
    fn parses_jetton_burn_body() {
        let wallet_address = addr(1);
        let owner = addr(2);
        let response_destination = addr(4);
        let mut body = CellBuilder::new();
        body.store_u32(JETTON_BURN_OPCODE).unwrap();
        body.store_u64(8).unwrap();
        body.store_var_bigint(&BigInt::from(43), 4, false).unwrap();
        store_address(&mut body, Some(response_destination));
        body.store_bit_zero().unwrap();
        let transaction = transaction(
            JETTON_BURN_OPCODE,
            wallet_address,
            owner,
            body.build().unwrap(),
        );

        let event = parse_jetton_burn(&transaction, &wallet(wallet_address, owner, addr(5)))
            .unwrap()
            .unwrap();
        assert_eq!(event.query_id, "8");
        assert_eq!(event.amount, "43");
        assert_eq!(event.response_destination, Some(response_destination));
    }

    #[test]
    fn parses_nft_transfer_body() {
        let item_address = addr(1);
        let old_owner = addr(2);
        let new_owner = addr(3);
        let response_destination = addr(4);
        let mut body = CellBuilder::new();
        body.store_u32(NFT_TRANSFER_OPCODE).unwrap();
        body.store_u64(9).unwrap();
        store_address(&mut body, Some(new_owner));
        store_address(&mut body, Some(response_destination));
        body.store_bit_zero().unwrap();
        body.store_var_bigint(&BigInt::from(6), 4, false).unwrap();
        body.store_bit_zero().unwrap();
        let transaction = transaction(
            NFT_TRANSFER_OPCODE,
            item_address,
            old_owner,
            body.build().unwrap(),
        );
        let item = NftItemMeta {
            address: item_address,
            code_hash: Hash256([1; 32]),
            data_hash: Hash256([2; 32]),
            collection_address: Some(addr(5)),
            owner_address: Some(new_owner),
            content: Value::Null,
            index: "1".to_owned(),
            init: true,
            last_transaction_lt: 456,
        };

        let event = parse_nft_transfer(&transaction, &item).unwrap().unwrap();
        assert_eq!(event.query_id, "9");
        assert_eq!(event.old_owner, old_owner);
        assert_eq!(event.new_owner, new_owner);
        assert_eq!(event.forward_amount, "6");
    }
}
