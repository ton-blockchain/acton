use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tvm_ffi::stack::Tuple;
use tycho_types::cell::Cell;
use tycho_types::dict::Dict;
use tycho_types::prelude::HashBytes;

pub(crate) fn parse_token_content(content_cell: Cell, keys: &[&str]) -> Value {
    let Ok(mut parser) = content_cell.as_slice() else {
        return json!({});
    };

    let Ok(prefix) = parser.load_uint(8) else {
        return json!({});
    };

    if prefix == 0x01 {
        let mut remaining = parser.load_remaining();
        if let Some(uri) = Tuple::parse_snake_string_slice(&mut remaining) {
            return json!({ "uri": uri });
        }
    } else if prefix == 0x00 {
        return parse_onchain_content(content_cell, keys);
    }

    json!({})
}

fn parse_onchain_content(content_cell: Cell, keys: &[&str]) -> Value {
    let Ok(dict_cell) = content_cell.as_slice_allow_exotic().load_reference_cloned() else {
        return json!({});
    };

    let dict: Dict<HashBytes, Cell> = Dict::from_raw(Some(dict_cell));
    let mut map = serde_json::Map::new();

    for &key_name in keys {
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

    Value::Object(map)
}
