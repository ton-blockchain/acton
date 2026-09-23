//! Asynchronous access to TON Center using the shared API response models.

#[cfg(test)]
mod tests;

use crate::Network;
use anyhow::Context;
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use std::collections::HashMap;
use std::env;
use std::ffi::OsStr;
use ton_executor::message::{PrevBlockId, PrevBlocksInfo};
use ton_networks::CustomNetworkUrls;
use toncenter::v2;
use toncenter_client::{Client, V2Transport};
use toncenter_keys::api_key as toncenter_api_key;

const USE_PROXY_ENV: &str = "ACTON_USE_PROXY";

const fn user_agent() -> &'static str {
    concat!("acton/", env!("CARGO_PKG_VERSION"))
}

fn proxy_enabled() -> bool {
    proxy_enabled_from_value(env::var_os(USE_PROXY_ENV).as_deref())
}

fn proxy_enabled_from_value(value: Option<&OsStr>) -> bool {
    value.is_some_and(|value| {
        let value = value.to_string_lossy();
        let value = value.trim();
        value == "1" || value == "true"
    })
}

/// Resolves both API endpoints through the shared network configuration.
/// V2 and V3 may use different origins or path prefixes; neither is derived
/// from the other. Custom networks must configure both APIs for replay.
pub(crate) fn client(
    network: Network,
    custom_networks: &HashMap<String, CustomNetworkUrls>,
) -> anyhow::Result<Client> {
    Ok(Client::builder()
        .user_agent(user_agent())
        .system_proxy(proxy_enabled())
        .v2_url(network.toncenter_v2_url(custom_networks)?)
        .v3_url(network.toncenter_v3_url(custom_networks)?)
        .api_key(toncenter_api_key(&network))
        .build()?)
}

/// Reconstructs c7 history at the referenced masterchain state, newest first.
/// The anchor is included, as is zerostate when fewer than 16 blocks exist.
/// Lookups are sequential to respect public API limits; overlapping lists
/// share results for the duration of this reconstruction.
pub(crate) async fn get_prev_blocks_info(
    client: &Client,
    mc_seqno: u32,
    with_100: bool,
) -> anyhow::Result<PrevBlocksInfo> {
    async {
        let mut blocks = HashMap::new();
        let key_seqno = if mc_seqno == 0 {
            0
        } else {
            let header: v2::responses::BlockHeader = client
                .v2_request(
                    V2Transport::Get,
                    "getBlockHeader",
                    &[
                        ("workchain", "-1".to_owned()),
                        ("shard", "8000000000000000".to_owned()),
                        ("seqno", mc_seqno.to_string()),
                    ],
                )
                .await?;
            let key_seqno = if header.is_key_block {
                mc_seqno
            } else {
                u32::try_from(header.prev_key_block_seqno)
                    .context("Invalid previous key block seqno")?
            };
            anyhow::ensure!(
                key_seqno <= mc_seqno,
                "Previous key block is after masterchain block {mc_seqno}"
            );
            blocks.insert(mc_seqno, prev_block_id(header.id, mc_seqno)?);
            key_seqno
        };

        let recent_seqnos = (mc_seqno.saturating_sub(15)..=mc_seqno)
            .rev()
            .collect::<Vec<_>>();
        let hundred_seqnos = if with_100 {
            (0..=mc_seqno / 100)
                .rev()
                .take(16)
                .map(|seqno| seqno * 100)
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };

        for seqno in std::iter::once(key_seqno)
            .chain(recent_seqnos.iter().copied())
            .chain(hundred_seqnos.iter().copied())
        {
            if blocks.contains_key(&seqno) {
                continue;
            }

            // lookupBlock cannot return zerostate; the network's initial ID
            // is available in getMasterchainInfo instead.
            let id = if seqno == 0 {
                let info: v2::responses::MasterchainInfo = client
                    .v2_request(
                        V2Transport::Get,
                        "getMasterchainInfo",
                        &v2::requests::EmptyRequest {},
                    )
                    .await?;
                info.init
            } else {
                client
                    .v2_request(
                        V2Transport::Get,
                        "lookupBlock",
                        &[
                            ("workchain", "-1".to_owned()),
                            ("shard", "8000000000000000".to_owned()),
                            ("seqno", seqno.to_string()),
                        ],
                    )
                    .await?
            };
            blocks.insert(seqno, prev_block_id(id, seqno)?);
        }

        Ok(PrevBlocksInfo::new(
            recent_seqnos
                .iter()
                .map(|seqno| blocks[seqno].clone())
                .collect(),
            blocks[&key_seqno].clone(),
            with_100.then(|| {
                hundred_seqnos
                    .iter()
                    .map(|seqno| blocks[seqno].clone())
                    .collect()
            }),
        ))
    }
    .await
    .with_context(|| format!("Failed to load previous blocks at masterchain block {mc_seqno}"))
}

/// Validates API block IDs before exposing their hashes to contract code in c7.
fn prev_block_id(
    id: v2::responses::TonBlockIdExt,
    expected_seqno: u32,
) -> anyhow::Result<PrevBlockId> {
    anyhow::ensure!(
        id.workchain == -1
            && (matches!(
                id.shard.as_str(),
                "-9223372036854775808" | "8000000000000000"
            ) || (expected_seqno == 0 && id.shard == "0"))
            && id.seqno == i64::from(expected_seqno),
        "TON Center returned an unexpected masterchain block for seqno {expected_seqno}"
    );
    let decode_hash = |value: &str| -> anyhow::Result<[u8; 32]> {
        STANDARD
            .decode(value)?
            .try_into()
            .map_err(|bytes: Vec<u8>| {
                anyhow::anyhow!(
                    "Invalid block hash length: expected 32 bytes, got {}",
                    bytes.len()
                )
            })
    };

    Ok(PrevBlockId {
        workchain: -1,
        shard: i64::MIN,
        seqno: expected_seqno,
        root_hash: decode_hash(&id.root_hash).context("Invalid masterchain root hash")?,
        file_hash: decode_hash(&id.file_hash).context("Invalid masterchain file hash")?,
    })
}
