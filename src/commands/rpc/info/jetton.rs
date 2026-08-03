use super::{
    InspectionDetails, InspectionReport, InspectorContext, JettonMasterInspection, JettonTokenJson,
    JettonWalletInspection, amount_json, hash_json, int_address_json, remote_get_method_libs,
    std_address_json,
};
use anyhow::Context;
use std::collections::{HashMap, HashSet};
use std::str::FromStr;
use tokio::task::JoinSet;
use ton_api::{Network, OffchainJsonResolver, TonApiClient};
use ton_indexer_contracts::jettons;
use tycho_types::cell::Cell;
use tycho_types::models::IntAddr;

pub(super) fn inspect(ctx: &InspectorContext<'_>, reports: &mut Vec<InspectionReport>) {
    let (Some(code), Some(data)) = (ctx.code, ctx.data) else {
        return;
    };

    if let Some(master) = detect_master(
        ctx.address.to_string(),
        code,
        data,
        ctx.network,
        ctx.get_method_libs,
    ) {
        reports.push(InspectionReport {
            kind: "jetton_master",
            confidence: "high",
            source: "ton-indexer-contracts:get_jetton_data",
            warnings: Vec::new(),
            details: InspectionDetails::JettonMaster(Box::new(master)),
        });
    }

    if let Some(wallet) = detect_wallet(ctx, code, data) {
        reports.push(wallet);
    }
}

pub(super) async fn enrich_metadata(
    reports: &mut [InspectionReport],
    resolver: &OffchainJsonResolver,
) {
    let uris = reports
        .iter()
        .filter_map(|report| match &report.details {
            InspectionDetails::JettonMaster(master) => Some(&master.metadata),
            InspectionDetails::JettonWallet(wallet) => {
                wallet.token.as_ref().map(|token| &token.metadata)
            }
            InspectionDetails::MultisigWallet(_) => None,
        })
        .filter_map(|metadata| jettons::jetton_content_uri(metadata).map(str::to_owned))
        .collect::<HashSet<_>>();
    let mut pending = JoinSet::new();
    for uri in uris {
        let resolver = resolver.clone();
        pending.spawn(async move {
            let result = resolver
                .get_json(&uri)
                .await
                .map_err(|error| format!("{error:#}"));
            (uri, result)
        });
    }
    let mut resolved = HashMap::new();
    while let Some(result) = pending.join_next().await {
        match result {
            Ok((uri, result)) => {
                resolved.insert(uri, result);
            }
            Err(error) => log::debug!("Off-chain metadata task failed: {error}"),
        }
    }

    for report in reports {
        let metadata = match &mut report.details {
            InspectionDetails::JettonMaster(master) => &mut master.metadata,
            InspectionDetails::JettonWallet(wallet) => {
                let Some(token) = &mut wallet.token else {
                    continue;
                };
                &mut token.metadata
            }
            InspectionDetails::MultisigWallet(_) => continue,
        };
        let Some(uri) = jettons::jetton_content_uri(metadata).map(str::to_owned) else {
            continue;
        };

        match resolved.get(&uri) {
            Some(Ok(remote_metadata)) => {
                jettons::merge_jetton_content(metadata, remote_metadata);
            }
            Some(Err(error)) => report
                .warnings
                .push(format!("failed to load off-chain jetton metadata: {error}")),
            None => report
                .warnings
                .push("failed to load off-chain jetton metadata".to_owned()),
        }
    }
}

fn detect_wallet(ctx: &InspectorContext<'_>, code: &Cell, data: &Cell) -> Option<InspectionReport> {
    let wallet_data = jettons::get_jetton_wallet_data(
        ctx.address.to_string(),
        code.clone(),
        data.clone(),
        ctx.get_method_libs,
    )?;

    let mut warnings = Vec::new();
    let token = load_master_for_wallet(ctx, &wallet_data.jetton_master_address, &mut warnings).map(
        |master| JettonTokenJson {
            master_address: master.address,
            metadata: master.metadata.clone(),
            total_supply: master.total_supply,
            mintable: master.mintable,
            admin_address: master.admin_address,
            wallet_code_hash: master.wallet_code_hash,
        },
    );
    let token_metadata = token.as_ref().map(|token| &token.metadata);

    Some(InspectionReport {
        kind: "jetton_wallet",
        confidence: "high",
        source: "ton-indexer-contracts:get_wallet_data",
        warnings,
        details: InspectionDetails::JettonWallet(Box::new(JettonWalletInspection {
            address: std_address_json(ctx.address, ctx.network),
            balance: amount_json(&wallet_data.balance, token_metadata),
            owner_address: int_address_json(&wallet_data.owner_address, ctx.network),
            master_address: int_address_json(&wallet_data.jetton_master_address, ctx.network),
            wallet_code_hash: hash_json(wallet_data.jetton_wallet_code.repr_hash()),
            token,
        })),
    })
}

fn load_master_for_wallet(
    ctx: &InspectorContext<'_>,
    master_address: &IntAddr,
    warnings: &mut Vec<String>,
) -> Option<JettonMasterInspection> {
    let remote = ctx
        .client
        .get_account_info(ctx.block_number, &master_address.to_string())
        .with_context(|| format!("failed to fetch jetton master {master_address}"))
        .map_err(|err| warnings.push(format!("{err:#}")))
        .ok()?;
    let code = TonApiClient::decode_optional_cell(&remote.code)
        .with_context(|| format!("failed to decode jetton master code {master_address}"))
        .map_err(|err| warnings.push(format!("{err:#}")))
        .ok()
        .flatten()?;
    let data = TonApiClient::decode_optional_cell(&remote.data)
        .with_context(|| format!("failed to decode jetton master data {master_address}"))
        .map_err(|err| warnings.push(format!("{err:#}")))
        .ok()
        .flatten()?;
    let libs = match master_address.as_std() {
        Some(address) => match remote_get_method_libs(ctx.client, address, &code) {
            Ok(libs) => libs,
            Err(err) => {
                warnings.push(format!("{err:#}"));
                None
            }
        },
        None => None,
    };

    detect_master(
        master_address.to_string(),
        &code,
        &data,
        ctx.network,
        libs.as_deref(),
    )
    .or_else(|| {
        warnings.push(format!(
            "account {master_address} did not match jetton master get-method shape"
        ));
        None
    })
}

fn detect_master(
    address: String,
    code: &Cell,
    data: &Cell,
    network: &Network,
    libs: Option<&str>,
) -> Option<JettonMasterInspection> {
    let jetton_data = jettons::get_jetton_data(address.clone(), code.clone(), data.clone(), libs)?;
    let metadata = jettons::parse_jetton_content(jetton_data.jetton_content);

    Some(JettonMasterInspection {
        address: int_address_json(&IntAddr::from_str(&address).ok()?, network),
        total_supply: amount_json(&jetton_data.total_supply, Some(&metadata)),
        mintable: jetton_data.mintable,
        admin_address: jetton_data
            .admin_address
            .as_ref()
            .map(|address| int_address_json(address, network)),
        metadata,
        wallet_code_hash: hash_json(jetton_data.jetton_wallet_code.repr_hash()),
    })
}
