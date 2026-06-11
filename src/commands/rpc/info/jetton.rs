use super::{
    InspectionDetails, InspectionReport, InspectorContext, JettonMasterInspection, JettonTokenJson,
    JettonWalletInspection, amount_json, hash_json, int_address_json, remote_get_method_libs,
    std_address_json,
};
use anyhow::Context;
use std::str::FromStr;
use ton_api::{Network, TonApiClient};
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
            source: "ton-indexer:get_jetton_data",
            warnings: Vec::new(),
            details: InspectionDetails::JettonMaster(Box::new(master)),
        });
    }

    if let Some(wallet) = detect_wallet(ctx, code, data) {
        reports.push(wallet);
    }
}

fn detect_wallet(ctx: &InspectorContext<'_>, code: &Cell, data: &Cell) -> Option<InspectionReport> {
    let wallet_data = ton_indexer::jettons::get_jetton_wallet_data(
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
        source: "ton-indexer:get_wallet_data",
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
    let jetton_data =
        ton_indexer::jettons::get_jetton_data(address.clone(), code.clone(), data.clone(), libs)?;
    let metadata = ton_indexer::jettons::resolve_jetton_content(
        ton_indexer::jettons::parse_jetton_content(jetton_data.jetton_content),
    );

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
