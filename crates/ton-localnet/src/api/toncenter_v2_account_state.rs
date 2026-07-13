use crate::api::toncenter_wallet::{
    V2WalletState, account_code_hash, read_v2_extended_wallet_state,
};
use crate::localnet::LocalnetAccountState;
use crate::types::Hash256;
use anyhow::Context;
use tycho_types::boc::Boc;

const HIGHLOAD_V1_CODE_HASHES: &[(&str, i32)] = &[
    ("CrH/k6nnnA3v9EBdja1rWsn4wBsvHry0JZ/rnpg4AJk=", -1),
    ("2M27t58sXKpnesRQdwvgNRviHhJQSG3oXMUqoz3RZIQ=", 1),
    ("NowDly5fTCRBK4GGUt4d4qJjDRwBFgmZnf90X7NoSa0=", 2),
];
const HIGHLOAD_V2_CODE_HASHES: &[(&str, i32)] = &[
    ("lJTRzI7fEvBWcaGpugmSEJbrUIEeGSTsZcPGKfu4CBI=", -1),
    ("jOtFs81LXMYOquHBO5wJI5Jnf+U2sumy2AG2Lv+TH+E=", 1),
    ("CzqIeurNKn1Au1VQvJJTFWoCkGWu+21rWDc11Y2p1b4=", 2),
];
const MANUAL_DNS_CODE_HASHES: &[(&str, i32)] = &[
    ("c1Ot4+H3y+3FJbPnvDIm/lrIJ2+lJ5rDf7bvvIj0Irc=", -1),
    ("oNZ9LeiRn+e3yndlLM1P3FbGid56aRYeRfsNzF5HwVU=", 1),
];

#[derive(Debug, Clone, Copy)]
pub(crate) enum V2ExtendedAccountState {
    Standard(V2WalletState),
    HighloadV1 {
        revision: i32,
        wallet_id: u32,
        seqno: u32,
    },
    HighloadV2 {
        revision: i32,
        wallet_id: u32,
    },
    Dns {
        revision: i32,
        wallet_id: u32,
    },
}

pub(crate) fn read_v2_extended_account_state(
    account: &LocalnetAccountState,
) -> anyhow::Result<Option<V2ExtendedAccountState>> {
    if let Some(wallet) = read_v2_extended_wallet_state(account)? {
        return Ok(Some(V2ExtendedAccountState::Standard(wallet)));
    }
    if account.code.is_none() {
        return Ok(None);
    }

    let Some(code) = classify_specialized_code(account_code_hash(account)?) else {
        return Ok(None);
    };

    match code {
        SpecializedCode::HighloadV1 { revision } => {
            let (seqno, wallet_id) = parse_account_data(account, "highload wallet v1", |data| {
                Ok((data.load_u32()?, data.load_u32()?))
            })?;
            Ok(Some(V2ExtendedAccountState::HighloadV1 {
                revision,
                wallet_id,
                seqno,
            }))
        }
        SpecializedCode::HighloadV2 { revision } => {
            let wallet_id = read_optional_wallet_id(account, "highload wallet v2")?;
            Ok(Some(V2ExtendedAccountState::HighloadV2 {
                revision,
                wallet_id,
            }))
        }
        SpecializedCode::Dns { revision } => {
            let wallet_id = read_optional_wallet_id(account, "manual DNS")?;
            Ok(Some(V2ExtendedAccountState::Dns {
                revision,
                wallet_id,
            }))
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SpecializedCode {
    HighloadV1 { revision: i32 },
    HighloadV2 { revision: i32 },
    Dns { revision: i32 },
}

fn classify_specialized_code(code_hash: Hash256) -> Option<SpecializedCode> {
    let code_hash = code_hash.to_base64();
    find_revision(HIGHLOAD_V1_CODE_HASHES, &code_hash)
        .map(|revision| SpecializedCode::HighloadV1 { revision })
        .or_else(|| {
            find_revision(HIGHLOAD_V2_CODE_HASHES, &code_hash)
                .map(|revision| SpecializedCode::HighloadV2 { revision })
        })
        .or_else(|| {
            find_revision(MANUAL_DNS_CODE_HASHES, &code_hash)
                .map(|revision| SpecializedCode::Dns { revision })
        })
}

fn find_revision(code_hashes: &[(&str, i32)], code_hash: &str) -> Option<i32> {
    code_hashes
        .iter()
        .find_map(|(known_hash, revision)| (*known_hash == code_hash).then_some(*revision))
}

fn read_optional_wallet_id(
    account: &LocalnetAccountState,
    account_type: &'static str,
) -> anyhow::Result<u32> {
    if account.data.is_none() {
        return Ok(0);
    }
    parse_account_data(account, account_type, |data| {
        data.load_u32().map_err(Into::into)
    })
}

fn parse_account_data<T>(
    account: &LocalnetAccountState,
    account_type: &'static str,
    parse: impl FnOnce(&mut tycho_types::cell::CellSlice<'_>) -> anyhow::Result<T>,
) -> anyhow::Result<T> {
    let data = account
        .data
        .as_ref()
        .with_context(|| format!("{account_type} state has no data"))?;
    let cell =
        Boc::decode(data).with_context(|| format!("Failed to decode {account_type} data"))?;
    let mut data = cell
        .as_slice()
        .with_context(|| format!("Failed to load {account_type} data"))?;
    parse(&mut data).with_context(|| format!("Failed to parse {account_type} data"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn specialized_code_hashes_match_pinned_tonlib_revisions() {
        let cases = [
            (
                "CrH/k6nnnA3v9EBdja1rWsn4wBsvHry0JZ/rnpg4AJk=",
                SpecializedCode::HighloadV1 { revision: -1 },
            ),
            (
                "2M27t58sXKpnesRQdwvgNRviHhJQSG3oXMUqoz3RZIQ=",
                SpecializedCode::HighloadV1 { revision: 1 },
            ),
            (
                "NowDly5fTCRBK4GGUt4d4qJjDRwBFgmZnf90X7NoSa0=",
                SpecializedCode::HighloadV1 { revision: 2 },
            ),
            (
                "lJTRzI7fEvBWcaGpugmSEJbrUIEeGSTsZcPGKfu4CBI=",
                SpecializedCode::HighloadV2 { revision: -1 },
            ),
            (
                "jOtFs81LXMYOquHBO5wJI5Jnf+U2sumy2AG2Lv+TH+E=",
                SpecializedCode::HighloadV2 { revision: 1 },
            ),
            (
                "CzqIeurNKn1Au1VQvJJTFWoCkGWu+21rWDc11Y2p1b4=",
                SpecializedCode::HighloadV2 { revision: 2 },
            ),
            (
                "c1Ot4+H3y+3FJbPnvDIm/lrIJ2+lJ5rDf7bvvIj0Irc=",
                SpecializedCode::Dns { revision: -1 },
            ),
            (
                "oNZ9LeiRn+e3yndlLM1P3FbGid56aRYeRfsNzF5HwVU=",
                SpecializedCode::Dns { revision: 1 },
            ),
        ];

        for (code_hash, expected) in cases {
            assert_eq!(
                classify_specialized_code(
                    Hash256::from_base64(code_hash).expect("code hash must decode")
                ),
                Some(expected),
            );
        }
        assert_eq!(classify_specialized_code(Hash256([0x55; 32])), None);
    }
}
