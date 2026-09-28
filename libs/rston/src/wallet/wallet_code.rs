use crate::boc::Boc;
use crate::cell::{Cell, HashBytes};
use crate::wallet::WalletVersion;
use crate::wallet::WalletVersion::*;
use std::collections::HashMap;
use std::sync::LazyLock;

macro_rules! load_code {
    ($filename:literal) => {
        Boc::decode_base64(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/resources/ton_wallet_code/",
            $filename
        )))
        .unwrap()
    };
}

pub static TON_WALLET_CODE_BY_VERSION: LazyLock<HashMap<WalletVersion, Cell>> =
    LazyLock::new(|| {
        #[allow(clippy::all)]
        HashMap::from([
            (V1R1, load_code!("wallet_v1r1.code")),
            (V1R2, load_code!("wallet_v1r2.code")),
            (V1R3, load_code!("wallet_v1r3.code")),
            (V2R1, load_code!("wallet_v2r1.code")),
            (V2R2, load_code!("wallet_v2r2.code")),
            (V3R1, load_code!("wallet_v3r1.code")),
            (V3R2, load_code!("wallet_v3r2.code")),
            (V4R1, load_code!("wallet_v4r1.code")),
            (V4R2, load_code!("wallet_v4r2.code")),
            (V5R1, load_code!("wallet_v5.code")),
            (HLV1R1, load_code!("highload_v1r1.code")),
            (HLV1R2, load_code!("highload_v1r2.code")),
            (HLV2, load_code!("highload_v2.code")),
            (HLV2R1, load_code!("highload_v2r1.code")),
            (HLV2R2, load_code!("highload_v2r2.code")),
        ])
    });

pub static TON_WALLET_VERSION_BY_CODE: LazyLock<HashMap<HashBytes, WalletVersion>> =
    LazyLock::new(|| {
        TON_WALLET_CODE_BY_VERSION
            .iter()
            .map(|(k, v)| (*v.repr_hash(), *k))
            .collect()
    });
