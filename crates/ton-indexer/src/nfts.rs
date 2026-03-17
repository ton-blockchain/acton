use num_bigint::BigInt;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tvmffi::stack::{Tuple, TupleItem};
use tycho_types::cell::{Cell, CellBuilder, Load};
use tycho_types::dict::Dict;
use tycho_types::models::IntAddr;
use tycho_types::prelude::HashBytes;

#[derive(Debug, Clone)]
pub struct NftItemData {
    pub init: bool,
    pub index: BigInt,
    pub collection_address: Option<String>,
    pub owner_address: Option<String>,
    pub individual_content: Cell,
}

pub fn get_nft_item_data(address: String, code: Cell, data: Cell) -> Option<NftItemData> {
    let Ok(result) = crate::jettons::run_get_method(address, code, data, "get_nft_data") else {
        return None;
    };

    if result.len() != 5 {
        return None;
    }

    let init = match &result[0] {
        TupleItem::Int(i) => i != &BigInt::from(0),
        _ => return None,
    };

    let index = match &result[1] {
        TupleItem::Int(i) => i.clone(),
        _ => return None,
    };

    let collection_address = parse_optional_address(&result[2]);
    let owner_address = parse_optional_address(&result[3]);

    let individual_content = match &result[4] {
        TupleItem::Cell(c) | TupleItem::Slice(c) => c.clone(),
        _ => return None,
    };

    Some(NftItemData {
        init,
        index,
        collection_address,
        owner_address,
        individual_content,
    })
}

fn parse_optional_address(item: &TupleItem) -> Option<String> {
    match item {
        TupleItem::Null => None,
        TupleItem::Slice(cell) | TupleItem::Cell(cell) => {
            let mut slice = cell.as_slice_allow_exotic();
            IntAddr::load_from(&mut slice)
                .ok()
                .map(|addr| addr.to_string())
        }
        _ => None,
    }
}

pub fn parse_nft_content(content_cell: Cell) -> Value {
    let mut parser = match content_cell.as_slice() {
        Ok(p) => p,
        Err(_) => return json!({}),
    };

    let prefix = match parser.load_uint(8) {
        Ok(p) => p,
        Err(_) => return json!({}),
    };

    if prefix == 0x01 {
        let remaining = parser.load_remaining();
        let mut builder = CellBuilder::new();
        if builder.store_slice(remaining).is_ok()
            && let Ok(cell) = builder.build()
            && let Some(uri) = Tuple::parse_snake_string(&cell)
        {
            return json!({ "uri": uri });
        }
    } else if prefix == 0x00 {
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
            "attributes",
            "cover_image",
            "animation_url",
            "external_url",
            "marketplace",
            "social_links",
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
