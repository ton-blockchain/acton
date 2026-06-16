use super::{
    decode_storage, find_local_contract_match, format_account_status, format_hash,
    format_int_address, format_std_address, load_rpc_config, print_get_method_hint,
    print_get_methods, print_indented_block, print_kv, print_section, print_section_title,
    resolve_rpc_network,
};
use crate::commands::common::{error_fmt, format_nanograms};
use crate::context::code_lookup_hash;
use acton_config::color::OwoColorize;
use anyhow::{Context, anyhow};
use log::warn;
use num_bigint::BigInt;
use serde::Serialize;
use serde_json::{Value, json};
use std::collections::HashSet;
use ton_api::{Network, TonApiClient, TonCenterAccountInfoResult};
use tycho_types::boc::Boc;
use tycho_types::cell::{Cell, HashBytes};
use tycho_types::dict::Dict;
use tycho_types::models::{IntAddr, LibDescr, StdAddr, StdAddrFormat};

mod jetton;
mod multisig;

const EXOTIC_LIBRARY_TAG: u8 = 2;

pub(super) fn rpc_info_cmd(
    address_input: &str,
    net: Option<String>,
    block_number: Option<u64>,
    json: bool,
    raw: bool,
) -> anyhow::Result<()> {
    let (address, _) = StdAddr::from_str_ext(address_input, StdAddrFormat::any())
        .map_err(|_| anyhow!("Invalid address"))
        .with_context(|| error_fmt::invalid_address(address_input))?;

    let network = resolve_rpc_network(net)?;
    let config = load_rpc_config()?;
    let client = TonApiClient::new(network.clone(), config.custom_networks())?;

    let remote = client
        .get_account_info(block_number, &address.to_string())
        .with_context(|| format!("Failed to fetch account info for {address} from {network}"))?;

    let balance = remote.balance.to_bigint()?;
    let code = TonApiClient::decode_optional_cell(&remote.code)?;
    let data = TonApiClient::decode_optional_cell(&remote.data)?;

    let matched_contract = code
        .as_ref()
        .map(|code| find_local_contract_match(code.repr_hash(), &config))
        .transpose()?
        .flatten();
    let catalog_contract = code.as_ref().and_then(|code| {
        acton_abi_catalog::find_contract_by_code_hash(&code_lookup_hash(code).to_string())
    });
    let local_abi = matched_contract
        .as_ref()
        .and_then(|contract| contract.abi.clone());
    let catalog_abi = catalog_contract.map(acton_abi_catalog::CatalogContract::abi);
    let contract_name = match (
        matched_contract.as_ref(),
        local_abi.is_some(),
        catalog_contract,
    ) {
        (Some(contract), true, _) => contract.contract_name.green().to_string(),
        (_, _, Some(contract)) => contract.display_name.green().to_string(),
        _ => "unknown".dimmed().to_string(),
    };
    let has_abi = local_abi.is_some() || catalog_abi.is_some();
    let contract_name_plain = match (
        matched_contract.as_ref(),
        local_abi.is_some(),
        catalog_contract,
    ) {
        (Some(contract), true, _) => contract.contract_name.clone(),
        (_, _, Some(contract)) => contract.display_name.clone(),
        _ => "unknown".to_owned(),
    };
    let contract_source = match (
        matched_contract.as_ref(),
        local_abi.is_some(),
        catalog_contract,
    ) {
        (Some(_), true, _) => "local",
        (_, _, Some(_)) => "catalog",
        (Some(_), false, _) => "local_code_hash",
        _ => "none",
    };
    let doc_abi_command = has_abi.then(|| format_doc_abi_command(&contract_name_plain));

    let decoded_storage = match (&data, local_abi.as_deref(), catalog_abi.as_deref()) {
        (Some(data), Some(abi), _) => Some(decode_storage(data, abi, &network)?),
        (Some(data), None, Some(abi)) => match decode_storage(data, abi, &network) {
            Ok(decoded_storage) => Some(decoded_storage),
            Err(err) => {
                warn!("Skipping bundled ABI storage decode: {err:#}");
                None
            }
        },
        _ => None,
    };
    let get_method_libs = match code.as_ref() {
        Some(code) => match remote_get_method_libs(&client, &address, code) {
            Ok(libs) => libs,
            Err(err) => {
                warn!("Skipping remote get-method libraries: {err:#}");
                None
            }
        },
        None => None,
    };
    let inspections = if raw {
        Vec::new()
    } else {
        inspect_account(&InspectorContext {
            address: &address,
            network: &network,
            block_number,
            client: &client,
            code: code.as_ref(),
            data: data.as_ref(),
            get_method_libs: get_method_libs.as_deref(),
        })
    };

    if json {
        let output = json_report(JsonReportInput {
            network: &network,
            address: &address,
            block_number,
            remote: &remote,
            balance: &balance,
            code: code.as_ref(),
            data: data.as_ref(),
            contract_name: &contract_name_plain,
            contract_source,
            has_abi,
            doc_abi_command: doc_abi_command.as_deref(),
            decoded_storage: decoded_storage.as_deref(),
            inspections: &inspections,
        })?;
        println!("{}", serde_json::to_string_pretty(&output)?);
        return Ok(());
    }

    print_section_title("Basic Information");
    print_kv("Network", network.to_string());
    if let Some(block_number) = block_number {
        print_kv("Block", block_number.to_string().yellow().to_string());
    }
    print_kv("Raw Address", address.to_string().cyan().to_string());
    print_kv("Status", format_account_status(&remote.state));
    print_kv("Contract", contract_name);
    print_kv("Balance", format_nanograms(&balance).white().to_string());
    print_kv("Last Tx LT", remote.last_transaction_id.lt.as_str());
    print_kv(
        "Last Tx Hash",
        remote
            .last_transaction_id
            .hash
            .as_str()
            .yellow()
            .to_string(),
    );

    if let Some(code) = &code {
        print_kv("Code Hash", format_hash(code.repr_hash()));
    }
    if let Some(data) = &data {
        print_kv("Data Hash", format_hash(data.repr_hash()));
    }
    if remote.state == "frozen" && !remote.frozen_hash.is_empty() {
        print_kv(
            "Frozen Hash",
            remote.frozen_hash.as_str().yellow().to_string(),
        );
    }

    for inspection in &inspections {
        print_inspection(inspection);
    }

    match decoded_storage {
        Some(decoded_storage) => {
            print_section("Storage");
            print_indented_block(&decoded_storage, 2);
        }
        None if data.is_some() && has_abi => {
            print_section("Storage");
            println!("  {}", "<unavailable>".dimmed());
        }
        None if data.is_some() => {
            print_section("Storage");
            println!("  {}", "<ABI not found>".dimmed());
        }
        None => {}
    }

    if let Some(abi) = local_abi.as_deref().or(catalog_abi.as_deref()) {
        print_section("Get Methods");
        print_get_methods(abi);
    }

    print_get_method_hint(address_input, &network, block_number);
    if let Some(command) = &doc_abi_command {
        print_doc_abi_hint(command);
    }

    Ok(())
}

pub(super) struct InspectorContext<'a> {
    pub(super) address: &'a StdAddr,
    pub(super) network: &'a Network,
    pub(super) block_number: Option<u64>,
    pub(super) client: &'a TonApiClient,
    pub(super) code: Option<&'a Cell>,
    pub(super) data: Option<&'a Cell>,
    pub(super) get_method_libs: Option<&'a str>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct InspectionReport {
    pub(super) kind: &'static str,
    pub(super) confidence: &'static str,
    pub(super) source: &'static str,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(super) warnings: Vec<String>,
    #[serde(flatten)]
    pub(super) details: InspectionDetails,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase", untagged)]
pub(super) enum InspectionDetails {
    JettonMaster(Box<JettonMasterInspection>),
    JettonWallet(Box<JettonWalletInspection>),
    MultisigWallet(Box<MultisigWalletInspection>),
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct JettonMasterInspection {
    pub(super) address: AddressJson,
    pub(super) total_supply: TokenAmountJson,
    pub(super) mintable: bool,
    pub(super) admin_address: Option<AddressJson>,
    pub(super) metadata: Value,
    pub(super) wallet_code_hash: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct JettonWalletInspection {
    pub(super) address: AddressJson,
    pub(super) balance: TokenAmountJson,
    pub(super) owner_address: AddressJson,
    pub(super) master_address: AddressJson,
    pub(super) wallet_code_hash: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) token: Option<JettonTokenJson>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct JettonTokenJson {
    pub(super) master_address: AddressJson,
    pub(super) metadata: Value,
    pub(super) total_supply: TokenAmountJson,
    pub(super) mintable: bool,
    pub(super) admin_address: Option<AddressJson>,
    pub(super) wallet_code_hash: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct MultisigWalletInspection {
    pub(super) address: AddressJson,
    pub(super) next_order_seqno: String,
    pub(super) allow_arbitrary_order_seqno: bool,
    pub(super) threshold: String,
    pub(super) signers: Vec<IndexedAddressJson>,
    pub(super) proposers: Vec<IndexedAddressJson>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct IndexedAddressJson {
    pub(super) index: u8,
    pub(super) address: AddressJson,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct TokenAmountJson {
    pub(super) raw: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) normalized: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) decimals: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) symbol: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct AddressJson {
    pub(super) raw: String,
    pub(super) friendly: String,
}

pub(super) struct JsonReportInput<'a> {
    pub(super) network: &'a Network,
    pub(super) address: &'a StdAddr,
    pub(super) block_number: Option<u64>,
    pub(super) remote: &'a TonCenterAccountInfoResult,
    pub(super) balance: &'a BigInt,
    pub(super) code: Option<&'a Cell>,
    pub(super) data: Option<&'a Cell>,
    pub(super) contract_name: &'a str,
    pub(super) contract_source: &'a str,
    pub(super) has_abi: bool,
    pub(super) doc_abi_command: Option<&'a str>,
    pub(super) decoded_storage: Option<&'a str>,
    pub(super) inspections: &'a [InspectionReport],
}

pub(super) fn inspect_account(ctx: &InspectorContext<'_>) -> Vec<InspectionReport> {
    let mut reports = Vec::new();
    jetton::inspect(ctx, &mut reports);
    multisig::inspect(ctx, &mut reports);
    reports
}

pub(super) fn remote_get_method_libs(
    client: &TonApiClient,
    owner: &StdAddr,
    code: &Cell,
) -> anyhow::Result<Option<String>> {
    let mut pending = collect_library_refs(code)?;
    if pending.is_empty() {
        return Ok(None);
    }

    let mut seen = HashSet::new();
    let mut libs = Dict::<HashBytes, LibDescr>::new();

    while let Some(hash) = pending.pop() {
        if !seen.insert(hash) {
            continue;
        }

        let lib = client
            .get_library_by_hash(&hash)
            .with_context(|| format!("failed to fetch library 0x{hash}"))?;
        if lib.repr_hash() != &hash {
            anyhow::bail!(
                "Fetched library hash mismatch: requested 0x{hash}, got {}",
                lib.repr_hash()
            );
        }

        pending.extend(collect_library_refs(&lib)?);

        let mut publishers = Dict::<HashBytes, ()>::new();
        publishers
            .add(owner.address, ())
            .context("Failed to add account owner to library publishers")?;
        libs.add(hash, LibDescr { lib, publishers })
            .context("Failed to add remote library to VM dictionary")?;
    }

    Ok(libs.into_root().map(|cell| Boc::encode_base64(&cell)))
}

fn collect_library_refs(root: &Cell) -> anyhow::Result<Vec<HashBytes>> {
    let mut hashes = HashSet::new();
    let mut visited = HashSet::new();
    collect_library_refs_inner(root, &mut hashes, &mut visited)?;
    Ok(hashes.into_iter().collect())
}

fn collect_library_refs_inner(
    cell: &Cell,
    hashes: &mut HashSet<HashBytes>,
    visited: &mut HashSet<HashBytes>,
) -> anyhow::Result<()> {
    if !visited.insert(*cell.repr_hash()) {
        return Ok(());
    }

    if let Some(hash) = library_ref_hash(cell)? {
        hashes.insert(hash);
    }

    for index in 0..cell.reference_count() {
        if let Some(child) = cell.reference_cloned(index) {
            collect_library_refs_inner(&child, hashes, visited)?;
        }
    }

    Ok(())
}

fn library_ref_hash(cell: &Cell) -> anyhow::Result<Option<HashBytes>> {
    if !cell.is_exotic() {
        return Ok(None);
    }

    let slice = cell.as_slice_allow_exotic();
    if slice.size_bits() != 8 + 256 {
        return Ok(None);
    }

    let mut slice = cell.as_slice_allow_exotic();
    if slice.load_u8()? != EXOTIC_LIBRARY_TAG {
        return Ok(None);
    }

    Ok(Some(slice.load_u256()?))
}

fn print_inspection(report: &InspectionReport) {
    match &report.details {
        InspectionDetails::JettonMaster(master) => print_jetton_master(master, &report.warnings),
        InspectionDetails::JettonWallet(wallet) => print_jetton_wallet(wallet, &report.warnings),
        InspectionDetails::MultisigWallet(wallet) => {
            print_multisig_wallet(wallet, &report.warnings);
        }
    }
}

pub(super) fn json_report(input: JsonReportInput<'_>) -> anyhow::Result<Value> {
    let mut output = json!({
        "network": input.network.to_string(),
        "address": format_std_address(input.address, input.network),
        "rawAddress": input.address.to_string(),
        "account": {
            "status": input.remote.state,
            "balanceNano": input.balance.to_str_radix(10),
            "balance": format_nanograms(input.balance),
            "lastTransaction": {
                "lt": input.remote.last_transaction_id.lt,
                "hash": input.remote.last_transaction_id.hash,
            },
        },
        "contract": {
            "name": input.contract_name,
            "source": input.contract_source,
            "hasAbi": input.has_abi,
        },
        "decodedStorage": input.decoded_storage,
        "inspections": input.inspections,
    });

    if let Some(block_number) = input.block_number {
        output["block"] = json!(block_number);
    }
    if let Some(command) = input.doc_abi_command {
        output["contract"]["docAbiCommand"] = json!(command);
    }
    if let Some(code) = input.code {
        output["account"]["codeHash"] = json!(hash_json(code.repr_hash()));
    }
    if let Some(data) = input.data {
        output["account"]["dataHash"] = json!(hash_json(data.repr_hash()));
    }
    if input.remote.state == "frozen" && !input.remote.frozen_hash.is_empty() {
        output["account"]["frozenHash"] = json!(input.remote.frozen_hash);
    }

    Ok(output)
}

fn format_doc_abi_command(contract_name: &str) -> String {
    format!("acton doc abi {}", shell_quote_arg(contract_name))
}

fn print_doc_abi_hint(command: &str) {
    println!("{}", format!("hint: To view full ABI: {command}").dimmed());
}

fn shell_quote_arg(arg: &str) -> String {
    if !arg.is_empty()
        && arg
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b'/'))
    {
        return arg.to_owned();
    }

    format!("'{}'", arg.replace('\'', "'\\''"))
}

fn print_jetton_master(master: &JettonMasterInspection, warnings: &[String]) {
    print_section("Jetton Master");
    print_token_identity(&master.metadata);
    print_kv("Mintable", format_bool(master.mintable));
    print_kv(
        "Admin",
        optional_address_label(master.admin_address.as_ref()),
    );
    print_metadata_summary(&master.metadata);
    print_warnings(warnings);
}

fn print_jetton_wallet(wallet: &JettonWalletInspection, warnings: &[String]) {
    print_section("Jetton Wallet");
    if let Some(token) = &wallet.token {
        print_token_identity(&token.metadata);
    }
    print_kv("Balance", format_token_amount(&wallet.balance));
    print_kv("Owner", format_address_label(&wallet.owner_address));
    print_kv("Master", format_address_label(&wallet.master_address));
    if let Some(token) = &wallet.token {
        print_metadata_summary(&token.metadata);
    }
    print_warnings(warnings);
}

fn print_multisig_wallet(wallet: &MultisigWalletInspection, warnings: &[String]) {
    print_section("Multisig Wallet");
    print_kv(
        "Threshold",
        format!(
            "{} of {}",
            wallet.threshold.green().bold(),
            wallet.signers.len().to_string().white()
        ),
    );
    print_kv("Next Order Seqno", format_multisig_seqno(wallet));
    print_indexed_addresses("Signers", &wallet.signers);
    print_indexed_addresses("Proposers", &wallet.proposers);
    print_warnings(warnings);
}

fn format_multisig_seqno(wallet: &MultisigWalletInspection) -> String {
    if wallet.allow_arbitrary_order_seqno {
        "Arbitrary".green().to_string()
    } else {
        wallet.next_order_seqno.white().to_string()
    }
}

fn print_indexed_addresses(label: &str, addresses: &[IndexedAddressJson]) {
    if addresses.is_empty() {
        print_kv(label, "<none>".dimmed().to_string());
        return;
    }

    print_kv(label, addresses.len().to_string().white().to_string());
    for entry in addresses {
        println!(
            "    {} {}",
            format!("[{}]", entry.index).dimmed(),
            format_address_label(&entry.address)
        );
    }
}

fn print_metadata_summary(metadata: &Value) {
    for key in ["decimals", "description", "uri"] {
        if let Some(value) = metadata.get(key).and_then(Value::as_str)
            && !value.is_empty()
        {
            print_kv(metadata_label(key), format_metadata_value(key, value));
        }
    }
}

fn print_warnings(warnings: &[String]) {
    for warning in warnings {
        print_kv("Warning", warning.yellow().to_string());
    }
}

fn metadata_label(key: &str) -> &'static str {
    match key {
        "decimals" => "Decimals",
        "description" => "Description",
        "uri" => "Content URI",
        _ => "Metadata",
    }
}

fn print_token_identity(metadata: &Value) {
    print_kv("Token", format_token_label(metadata));
}

fn token_label(metadata: &Value) -> String {
    let name = metadata.get("name").and_then(Value::as_str);
    let symbol = metadata.get("symbol").and_then(Value::as_str);
    match (name, symbol) {
        (Some(name), Some(symbol)) if !name.is_empty() && !symbol.is_empty() => {
            format!("{name} ({symbol})")
        }
        (Some(name), _) if !name.is_empty() => name.to_owned(),
        (_, Some(symbol)) if !symbol.is_empty() => symbol.to_owned(),
        _ => "<unknown>".to_owned(),
    }
}

fn format_token_label(metadata: &Value) -> String {
    let label = token_label(metadata);
    if label == "<unknown>" {
        label.dimmed().to_string()
    } else {
        label.green().bold().to_string()
    }
}

fn format_token_amount(amount: &TokenAmountJson) -> String {
    match (&amount.normalized, &amount.symbol) {
        (Some(normalized), Some(symbol)) => {
            format!("{} {}", normalized.white(), symbol.green().bold())
        }
        (Some(normalized), None) => normalized.white().to_string(),
        _ => amount.raw.white().to_string(),
    }
}

fn optional_address_label(address: Option<&AddressJson>) -> String {
    address.map_or_else(|| "<none>".dimmed().to_string(), format_address_label)
}

fn format_address_label(address: &AddressJson) -> String {
    address.friendly.cyan().to_string()
}

fn format_bool(value: bool) -> String {
    if value {
        "true".green().to_string()
    } else {
        "false".yellow().to_string()
    }
}

fn format_metadata_value(key: &str, value: &str) -> String {
    match key {
        "uri" => value.cyan().to_string(),
        "decimals" | "description" => value.white().to_string(),
        _ => value.to_owned(),
    }
}

fn amount_json(raw: &BigInt, metadata: Option<&Value>) -> TokenAmountJson {
    let decimals = metadata.and_then(decimals_from_metadata);
    let symbol = metadata
        .and_then(|metadata| metadata.get("symbol"))
        .and_then(Value::as_str)
        .filter(|symbol| !symbol.is_empty())
        .map(ToOwned::to_owned);

    TokenAmountJson {
        raw: raw.to_str_radix(10),
        normalized: decimals.map(|decimals| normalize_amount(raw, decimals)),
        decimals,
        symbol,
    }
}

fn decimals_from_metadata(metadata: &Value) -> Option<u32> {
    let decimals = metadata.get("decimals")?;
    match decimals {
        Value::String(value) => value.parse().ok(),
        Value::Number(value) => value.as_u64().and_then(|value| value.try_into().ok()),
        _ => None,
    }
}

fn normalize_amount(raw: &BigInt, decimals: u32) -> String {
    if decimals == 0 {
        return raw.to_str_radix(10);
    }

    let sign = if raw.sign() == num_bigint::Sign::Minus {
        "-"
    } else {
        ""
    };
    let digits = raw.magnitude().to_str_radix(10);
    let decimals = decimals as usize;
    if digits.len() <= decimals {
        let fractional = format!("{digits:0>decimals$}");
        return format!("{sign}0.{}", trim_fractional(&fractional));
    }

    let split_at = digits.len() - decimals;
    let integer = &digits[..split_at];
    let fractional = trim_fractional(&digits[split_at..]);
    if fractional == "0" {
        format!("{sign}{integer}")
    } else {
        format!("{sign}{integer}.{fractional}")
    }
}

fn trim_fractional(value: &str) -> String {
    let trimmed = value.trim_end_matches('0');
    if trimmed.is_empty() {
        "0".to_owned()
    } else {
        trimmed.to_owned()
    }
}

fn int_address_json(address: &IntAddr, network: &Network) -> AddressJson {
    AddressJson {
        raw: address.to_string(),
        friendly: format_int_address(address, network),
    }
}

fn std_address_json(address: &StdAddr, network: &Network) -> AddressJson {
    AddressJson {
        raw: address.to_string(),
        friendly: format_std_address(address, network),
    }
}

fn hash_json(hash: &HashBytes) -> String {
    format!("0x{hash}")
}
