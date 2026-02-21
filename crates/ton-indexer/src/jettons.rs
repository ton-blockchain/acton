use num_bigint::BigInt;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::time::UNIX_EPOCH;
use ton_executor::ExecutorVerbosity;
use ton_executor::get::{GetExecutor, GetMethodResult, RunGetMethodArgs};
use tvmffi::serde::serialize_tuple;
use tvmffi::stack::{Tuple, TupleItem};
use tycho_types::boc::Boc;
use tycho_types::cell::{Cell, CellBuilder, Load};
use tycho_types::dict::Dict;
use tycho_types::models::IntAddr;
use tycho_types::prelude::HashBytes;

#[derive(Debug, Clone)]
pub struct JettonData {
    pub total_supply: BigInt,
    pub mintable: bool,
    pub admin_address: String,
    pub jetton_content: Cell,
    pub jetton_wallet_code: Cell,
}

#[derive(Debug, Clone)]
pub struct JettonWalletData {
    pub balance: BigInt,
    pub owner_address: String,
    pub jetton_master_address: String,
    pub jetton_wallet_code: Cell,
}

pub fn get_jetton_wallet_data(address: String, code: Cell, data: Cell) -> Option<JettonWalletData> {
    let Ok(result) = run_get_method(address, code, data, "get_wallet_data") else {
        return None;
    };

    if result.len() != 4 {
        return None;
    }

    let balance = match &result[0] {
        TupleItem::Int(i) => i.clone(),
        _ => return None,
    };

    let owner_address = match &result[1] {
        TupleItem::Slice(s) => {
            let mut slice = s.as_slice_allow_exotic();
            IntAddr::load_from(&mut slice).ok()?.to_string()
        }
        _ => return None,
    };

    let jetton_master_address = match &result[2] {
        TupleItem::Slice(s) => {
            let mut slice = s.as_slice_allow_exotic();
            IntAddr::load_from(&mut slice).ok()?.to_string()
        }
        _ => return None,
    };

    let jetton_wallet_code = match &result[3] {
        TupleItem::Cell(c) => c.clone(),
        _ => return None,
    };

    Some(JettonWalletData {
        balance,
        owner_address,
        jetton_master_address,
        jetton_wallet_code,
    })
}

pub fn parse_jetton_content(content_cell: Cell) -> Value {
    let mut parser = match content_cell.as_slice() {
        Ok(p) => p,
        Err(_) => return json!({}),
    };

    let prefix = match parser.load_uint(8) {
        Ok(p) => p,
        Err(_) => return json!({}),
    };

    if prefix == 0x01 {
        // Off-chain: read URI
        let remaining = parser.load_remaining();
        let mut builder = CellBuilder::new();
        if builder.store_slice(&remaining).is_ok()
            && let Ok(cell) = builder.build()
            && let Some(uri) = Tuple::parse_snake_string(&cell)
        {
            return json!({ "uri": uri });
        }
    } else if prefix == 0x00 {
        // On-chain: HashmapE 256 ^Cell
        let Ok(dict_cell) = content_cell.as_slice_allow_exotic().load_reference_cloned() else {
            return json!({});
        };

        let dict: Dict<HashBytes, Cell> = Dict::from_raw(Some(dict_cell));
        let mut map = serde_json::Map::new();

        let keys = vec![
            "uri",
            "name",
            "description",
            "image",
            "image_data",
            "symbol",
            "decimals",
            "amount_style",
            "render_type",
        ];

        for key_name in keys {
            let mut hasher = Sha256::new();
            hasher.update(key_name.as_bytes());
            let key_hash = HashBytes(hasher.finalize().into());

            let Ok(Some(value_cell)) = dict.get(key_hash) else {
                continue;
            };

            let mut slice = value_cell.as_slice_allow_exotic();
            let _ = slice.load_uint(8);

            if let Some(s) = Tuple::parse_snake_string_slice(&mut slice) {
                map.insert(key_name.to_string(), Value::String(s));
            }
        }

        return Value::Object(map);
    }

    json!({})
}

pub fn get_jetton_data(address: String, code: Cell, data: Cell) -> Option<JettonData> {
    let Ok(result) = run_get_method(address, code, data, "get_jetton_data") else {
        return None;
    };

    if result.len() != 5 {
        return None;
    }

    let total_supply = match &result[0] {
        TupleItem::Int(i) => i.clone(),
        _ => return None,
    };

    let mintable = match &result[1] {
        TupleItem::Int(i) => i != &BigInt::from(0),
        _ => return None,
    };

    let admin_address = match &result[2] {
        TupleItem::Slice(s) => {
            let mut slice = s.as_slice_allow_exotic();
            let addr = IntAddr::load_from(&mut slice).ok()?;
            addr.to_string()
        }
        _ => return None,
    };

    let jetton_content = match &result[3] {
        TupleItem::Cell(c) => c.clone(),
        _ => return None,
    };

    let jetton_wallet_code = match &result[4] {
        TupleItem::Cell(c) => c.clone(),
        _ => return None,
    };

    Some(JettonData {
        total_supply,
        mintable,
        admin_address,
        jetton_content,
        jetton_wallet_code,
    })
}

pub fn run_get_method(
    address: String,
    code: Cell,
    data: Cell,
    name: &str,
) -> anyhow::Result<Tuple> {
    const CRC16: crc::Crc<u16> = crc::Crc::<u16>::new(&crc::CRC_16_XMODEM);

    let method_id = (i32::from(CRC16.checksum(name.as_bytes())) & 0xFFFF) | 0x10000;

    let now = std::time::SystemTime::now();
    let duration_since_epoch = now.duration_since(UNIX_EPOCH).expect("Time went backwards");

    let params = RunGetMethodArgs {
        code: Boc::encode_base64(code),
        data: Boc::encode_base64(data),
        verbosity: ExecutorVerbosity::Short,
        libs: "".to_owned(),
        address,
        unixtime: duration_since_epoch.as_secs().try_into()?,
        balance: "10".to_string(),
        rand_seed: "0000000000000000000000000000000000000000000000000000000000000000".to_string(),
        gas_limit: "0".to_string(),
        method_id,
        debug_enabled: true,
        extra_currencies: HashMap::new(),
        prev_blocks_info: None,
    };

    let executor = GetExecutor::new(&params)?;

    let stack = Tuple(vec![]);
    let stack = Boc::encode_base64(serialize_tuple(&stack)?);
    let result = executor.run_get_method(&stack, &params, None)?;

    match result {
        GetMethodResult::Success(result) => {
            if result.vm_exit_code != 0 {
                anyhow::bail!("VM exited with code {}", result.vm_exit_code);
            }

            let cell = Boc::decode_base64(result.stack.as_ref())?;

            Tuple::deserialize(&cell)
        }
        GetMethodResult::Error(error) => {
            anyhow::bail!("{}", error.error)
        }
    }
}
