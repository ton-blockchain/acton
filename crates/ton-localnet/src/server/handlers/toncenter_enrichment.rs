use crate::api::toncenter_v3 as v3;
use crate::localnet::{Localnet, LocalnetAddressInfo};
use crate::types::Addr;
use std::collections::{BTreeSet, HashMap};
use ton_api::toncenter::v3 as v3_types;
use ton_indexer::categorize_wallet;
use tycho_types::cell::HashBytes as CellHashBytes;

#[derive(Clone, Default)]
pub(super) struct AddressInfo {
    pub interfaces: BTreeSet<String>,
    interfaces_available: bool,
    pub token_info: Vec<v3_types::TokenInfo>,
    extra_jetton_masters: BTreeSet<Addr>,
}

pub(super) async fn load_address_infos(
    node: &Localnet,
    addresses: Vec<Addr>,
) -> anyhow::Result<HashMap<Addr, AddressInfo>> {
    Ok(node
        .get_address_infos(addresses)
        .await?
        .into_iter()
        .map(|info| (info.address, map_address_info(info)))
        .collect())
}

pub(super) fn map_address_book_row(address: Addr, info: &AddressInfo) -> v3_types::AddressBookRow {
    v3_types::AddressBookRow {
        user_friendly: Some(address.as_user_friendly()),
        domain: None,
        interfaces: info
            .interfaces_available
            .then(|| info.interfaces.iter().cloned().collect()),
    }
}

pub(super) async fn build_metadata_for_addresses(
    node: &Localnet,
    addresses: Vec<Addr>,
) -> anyhow::Result<v3_types::Metadata> {
    let infos = load_address_infos(node, addresses).await?;
    build_metadata_from_infos(node, &infos).await
}

pub(super) async fn build_extra_data_for_addresses(
    node: &Localnet,
    addresses: Vec<Addr>,
    include_address_book: bool,
    include_metadata: bool,
) -> anyhow::Result<(Option<v3_types::AddressBook>, Option<v3_types::Metadata>)> {
    let infos = load_address_infos(node, addresses).await?;
    let address_book = include_address_book.then(|| {
        infos
            .iter()
            .map(|(address, info)| (address.to_string(), map_address_book_row(*address, info)))
            .collect()
    });
    let metadata = if include_metadata {
        Some(build_metadata_from_infos(node, &infos).await?)
    } else {
        None
    };
    Ok((address_book, metadata))
}

async fn build_metadata_from_infos(
    node: &Localnet,
    infos: &HashMap<Addr, AddressInfo>,
) -> anyhow::Result<v3_types::Metadata> {
    let mut metadata = v3_types::Metadata::new();
    let mut pending_jetton_masters = BTreeSet::new();
    for (address, info) in infos {
        pending_jetton_masters.extend(info.extra_jetton_masters.iter().copied());
        if !info.token_info.is_empty() {
            metadata.insert(
                address.to_string(),
                v3_types::AddressMetadata {
                    is_indexed: true,
                    token_info: info.token_info.clone(),
                },
            );
        }
    }

    let missing_master_addresses = pending_jetton_masters
        .into_iter()
        .filter(|address| !metadata.contains_key(&address.to_string()))
        .collect::<Vec<_>>();
    for (address, info) in load_address_infos(node, missing_master_addresses).await? {
        if !info.token_info.is_empty() {
            metadata.insert(
                address.to_string(),
                v3_types::AddressMetadata {
                    is_indexed: true,
                    token_info: info.token_info,
                },
            );
        }
    }

    Ok(metadata)
}

pub(super) fn map_address_info(info: LocalnetAddressInfo) -> AddressInfo {
    let mut out = AddressInfo::default();

    if let Some(code_hash) = info.code_hash {
        out.interfaces_available = true;
        let wallet_type = categorize_wallet(CellHashBytes(code_hash.0));
        if let Some(interface_name) = wallet_type.interface_name() {
            out.interfaces.insert(interface_name.to_string());
        }
    }

    if let Some(wallet) = info.jetton_wallet {
        out.interfaces_available = true;
        out.interfaces.insert("jetton_wallet".to_owned());
        out.token_info
            .push(v3::map_jetton_wallet_token_info(&wallet));
        out.extra_jetton_masters.insert(wallet.jetton_address);
    }

    if let Some(master) = info.jetton_master {
        out.interfaces_available = true;
        out.interfaces.insert("jetton_master".to_owned());
        out.token_info
            .push(v3::map_jetton_master_token_info(&master));
    }

    if let Some(item) = info.nft_item {
        out.interfaces_available = true;
        out.interfaces.insert("nft_item".to_owned());
        out.token_info.push(v3::map_nft_item_token_info(&item));
    }

    if let Some(item) = info.nft_collection_item {
        out.interfaces_available = true;
        out.interfaces.insert("nft_collection".to_owned());
        out.token_info
            .push(v3::map_nft_collection_token_info(&item));
    }

    out
}
