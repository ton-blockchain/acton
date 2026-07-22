use super::{
    LocalContractMatch, find_local_contract_match, format_get_method_signature,
    format_get_method_signature_colored, format_int_address, format_std_address, load_rpc_config,
    pretty_address_format, resolve_rpc_network,
};
use crate::commands::abi_args::{parse_abi_parameters, parse_number, parse_raw_stack_args};
use crate::commands::common::error_fmt;
use crate::context::code_lookup_hash;
use crate::formatter::FormatterContext;
use acton_config::color::{OwoColorize, colors_enabled};
use acton_config::config::ActonConfig;
#[cfg(test)]
use acton_debug::PrettyAddressFormat;
use acton_debug::{PrettyRenderOptions, RenderedValue, render_tuple_as_tolk_type};
use anyhow::{Context, anyhow};
use log::warn;
use num_traits::ToPrimitive;
use std::io::{Write, stderr, stdout};
use std::process;
use tolk_compiler::abi::{ABIGetMethod, ContractABI};
use tolk_compiler::types_kernel::{TyIdx, calc_width_on_stack};
use ton_api::{Network, TonApiClient};
use tvm_ffi::json_stack::legacy_stack_to_json;
use tvm_ffi::stack::{Tuple, TupleItem};
use tycho_types::boc::Boc;
use tycho_types::cell::Cell;
use tycho_types::models::{AnyAddr, IntAddr, StdAddr, StdAddrFormat};

pub(super) fn rpc_call_cmd(
    address: &str,
    method: &str,
    args: &[String],
    net: Option<String>,
    block_number: Option<u64>,
    json: bool,
    raw: bool,
) -> anyhow::Result<()> {
    let (address, _) = StdAddr::from_str_ext(address, StdAddrFormat::any())
        .map_err(|_| anyhow!("Invalid address"))
        .with_context(|| error_fmt::invalid_address(address))?;

    let network = resolve_rpc_network(net)?;
    let config = load_rpc_config()?;
    let client = TonApiClient::new(network.clone(), config.custom_networks())?;

    let remote = client
        .get_account_info(block_number, &address.to_string())
        .with_context(|| format!("Failed to fetch account info for {address} from {network}"))?;
    let code = TonApiClient::decode_optional_cell(&remote.code)?;
    let contract_match = code
        .as_ref()
        .map(|code| find_contract_match_for_rpc_call(code, &config))
        .transpose()?
        .flatten();

    let abi = contract_match
        .as_ref()
        .and_then(|matched| matched.abi.as_deref());
    let get_method = abi
        .map(|abi| resolve_get_method(abi, method))
        .transpose()?
        .flatten();

    let stack = if let (Some(abi), Some(get_method)) = (abi, get_method) {
        parse_get_method_parameters(abi, get_method, args)?
    } else {
        parse_raw_stack_args(args)?
    };
    let stack_json = legacy_stack_to_json(&stack).context("Failed to encode get-method stack")?;

    let result = client
        .run_get_method_at_block(&address.to_string(), method, &stack_json, block_number)
        .with_context(|| {
            format!("Failed to run get method {method} on {address} from {network}")
        })?;
    if json && result.exit_code != 0 {
        let mut output = serde_json::json!({
            "network": network.to_string(),
            "address": format_std_address(&address, &network),
            "rawAddress": address.to_string(),
            "contract": contract_match.as_ref().map(|matched| matched.contract_name.as_str()),
            "method": method,
            "signature": abi.zip(get_method).map(|(abi, method)| format_get_method_signature(abi, method)),
            "exitCode": result.exit_code,
            "error": get_method_exit_error_json(method, result.exit_code, abi),
            "result": serde_json::Value::Null,
            "rawStack": &result.stack,
        });
        if let Some(block_number) = block_number {
            output["block"] = serde_json::json!(block_number);
        }
        println!("{}", serde_json::to_string_pretty(&output)?);
        let _ = stdout().flush();
        let _ = stderr().flush();
        process::exit(1);
    }
    if !json && result.exit_code != 0 {
        println!(
            "{} {}",
            "Error:".red(),
            get_method_exit_error(method, result.exit_code, abi)
        );
        let _ = stdout().flush();
        let _ = stderr().flush();
        process::exit(1);
    }

    let result_tuple = result
        .parse_stack_tuple()
        .context("Failed to parse runGetMethod result stack")?;
    let decoded_result = if raw {
        None
    } else {
        abi.zip(get_method).and_then(|(abi, get_method)| {
            decode_get_method_result(&result_tuple, abi, get_method.return_ty_idx, &network)
                .map_err(|err| warn!("Skipping ABI result decode: {err:#}"))
                .ok()
        })
    };

    if json {
        let mut output = serde_json::json!({
            "network": network.to_string(),
            "address": format_std_address(&address, &network),
            "rawAddress": address.to_string(),
            "contract": contract_match.as_ref().map(|matched| matched.contract_name.as_str()),
            "method": method,
            "signature": abi.zip(get_method).map(|(abi, method)| format_get_method_signature(abi, method)),
            "exitCode": result.exit_code,
            "result": decoded_result.as_ref().map(|result| result.json.clone()),
            "rawStack": &result.stack,
        });
        if let Some(block_number) = block_number {
            output["block"] = serde_json::json!(block_number);
        }
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        match decoded_result {
            Some(decoded_result) => {
                print_pretty_result(&decoded_result.pretty);
            }
            None => {
                print_raw_stack(&result_tuple, &network);
            }
        }
    }

    if result.exit_code != 0 {
        anyhow::bail!("{}", get_method_exit_error(method, result.exit_code, abi));
    }

    Ok(())
}

fn find_contract_match_for_rpc_call(
    code: &Cell,
    config: &ActonConfig,
) -> anyhow::Result<Option<LocalContractMatch>> {
    let local_match = find_local_contract_match(code.repr_hash(), config)?;
    if local_match
        .as_ref()
        .is_some_and(|matched| matched.abi.is_some())
    {
        return Ok(local_match);
    }

    if let Some(catalog_contract) =
        acton_abi_catalog::find_contract_by_code_hash(&code_lookup_hash(code).to_string())
    {
        return Ok(Some(LocalContractMatch {
            contract_name: catalog_contract.display_name.clone(),
            abi: Some(catalog_contract.abi()),
        }));
    }

    Ok(local_match)
}

fn resolve_get_method<'a>(
    abi: &'a ContractABI,
    method: &str,
) -> anyhow::Result<Option<&'a ABIGetMethod>> {
    if let Some(get_method) = abi
        .get_methods
        .iter()
        .find(|get_method| get_method.name == method)
    {
        return Ok(Some(get_method));
    }

    if let Some(method_id) = parse_get_method_id(method) {
        return Ok(abi.find_get_method_by_id(method_id));
    }

    let available = abi
        .get_methods
        .iter()
        .map(|method| format!(" {}", format_get_method_signature_colored(abi, method)))
        .collect::<Vec<_>>()
        .join("\n");

    anyhow::bail!(
        "Get method {} not found in ABI for {}\nAvailable get methods:\n{}",
        method.yellow(),
        abi.contract_name.green(),
        if available.is_empty() {
            " none".dimmed().to_string()
        } else {
            available
        }
    )
}

fn parse_get_method_id(method: &str) -> Option<i32> {
    parse_number(method)?.to_i32()
}

fn parse_get_method_parameters(
    abi: &ContractABI,
    get_method: &ABIGetMethod,
    args: &[String],
) -> anyhow::Result<Tuple> {
    let expected_count = get_method.parameters.len();
    if args.len() != expected_count {
        anyhow::bail!(
            "Wrong number of arguments for {}: expected {}, got {}",
            format_get_method_signature_colored(abi, get_method),
            expected_count,
            args.len()
        );
    }

    parse_abi_parameters(abi, &get_method.parameters, args)
}

struct DecodedResult {
    json: serde_json::Value,
    pretty: String,
}

fn decode_get_method_result(
    tuple: &Tuple,
    abi: &ContractABI,
    return_ty_idx: TyIdx,
    network: &Network,
) -> anyhow::Result<DecodedResult> {
    let expected_width = calc_width_on_stack(abi, return_ty_idx);
    if expected_width != tuple.len() {
        anyhow::bail!(
            "Get-method result stack width mismatch for {}: expected {}, got {}",
            abi.render_type(return_ty_idx),
            expected_width,
            tuple.len()
        );
    }

    if expected_width == 0 {
        return Ok(DecodedResult {
            json: serde_json::Value::Null,
            pretty: "null".to_owned(),
        });
    }

    let rendered = render_tuple_as_tolk_type(abi, tuple, return_ty_idx);
    let options = PrettyRenderOptions {
        address_format: pretty_address_format(network),
        address_labels: Default::default(),
        colorize: colors_enabled(),
    };
    Ok(DecodedResult {
        json: rendered_value_to_json(&rendered, network),
        pretty: rendered.to_pretty_string(options),
    })
}

fn rendered_value_to_json(value: &RenderedValue, network: &Network) -> serde_json::Value {
    match value {
        RenderedValue::Leaf { value, type_field } => {
            leaf_value_to_json(value, type_field.as_deref())
        }
        RenderedValue::Address { value, fields, .. } => {
            serde_json::Value::String(rendered_address_value(value, fields, network))
        }
        RenderedValue::Struct { fields, .. } | RenderedValue::MapKV { fields, .. } => {
            fields_to_json(fields, network)
        }
        RenderedValue::Tensor { items, .. } | RenderedValue::ArrayOf { items, .. } => {
            serde_json::Value::Array(
                items
                    .iter()
                    .map(|item| rendered_value_to_json(item, network))
                    .collect(),
            )
        }
        RenderedValue::CellLike { value, fields, .. }
        | RenderedValue::CellOf { value, fields, .. } => {
            let mut object = serde_json::Map::new();
            object.insert("value".to_owned(), serde_json::Value::String(value.clone()));
            if !fields.is_empty() {
                object.insert("fields".to_owned(), fields_to_json(fields, network));
            }
            serde_json::Value::Object(object)
        }
        RenderedValue::EnumValue { value, .. } => serde_json::Value::String(value.clone()),
        RenderedValue::UnionCase {
            variant_name,
            fields,
            ..
        } => {
            let mut object = serde_json::Map::new();
            object.insert(
                "variant".to_owned(),
                serde_json::Value::String(variant_name.clone()),
            );
            if !fields.is_empty() {
                object.insert("fields".to_owned(), fields_to_json(fields, network));
            }
            serde_json::Value::Object(object)
        }
        RenderedValue::LastSeen { inner } => rendered_value_to_json(inner, network),
        RenderedValue::LazyNotYetLoaded { preview } => rendered_value_to_json(preview, network),
        RenderedValue::OptimizedOut => serde_json::Value::String("<optimized out>".to_owned()),
        RenderedValue::LazyCantParseSlice => serde_json::Value::String("<not loaded>".to_owned()),
        RenderedValue::LazyUnresolved { type_name } => {
            serde_json::Value::String(format!("{type_name} (lazy, unresolved)"))
        }
    }
}

fn fields_to_json(fields: &[(String, RenderedValue)], network: &Network) -> serde_json::Value {
    let mut object = serde_json::Map::new();
    for (name, value) in fields {
        object.insert(name.clone(), rendered_value_to_json(value, network));
    }
    serde_json::Value::Object(object)
}

fn leaf_value_to_json(value: &str, type_field: Option<&str>) -> serde_json::Value {
    if value == "null" || value == "()" {
        return serde_json::Value::Null;
    }
    if type_field == Some("bool") {
        return match value {
            "true" => serde_json::Value::Bool(true),
            "false" => serde_json::Value::Bool(false),
            _ => serde_json::Value::String(value.to_owned()),
        };
    }
    if type_field == Some("string")
        && let Some(unquoted) = value.strip_prefix('"').and_then(|s| s.strip_suffix('"'))
    {
        return serde_json::Value::String(unquoted.to_owned());
    }
    serde_json::Value::String(value.to_owned())
}

fn rendered_address_value(
    fallback: &str,
    fields: &[(String, RenderedValue)],
    network: &Network,
) -> String {
    let field_name = if network.uses_testnet_address_format() {
        "testnet"
    } else {
        "mainnet"
    };
    fields
        .iter()
        .find_map(|(name, value)| {
            if name == field_name
                && let RenderedValue::Leaf { value, .. } = value
            {
                return Some(value.clone());
            }
            None
        })
        .unwrap_or_else(|| fallback.to_owned())
}

fn print_pretty_result(value: &str) {
    for line in value.lines() {
        println!("{line}");
    }
}

struct RawStackRow {
    field: String,
    type_name: &'static str,
    value: String,
}

fn print_raw_stack(tuple: &Tuple, network: &Network) {
    let rows = tuple
        .iter()
        .enumerate()
        .map(|(index, item)| {
            let field_index = index + 1;
            let (type_name, value) = format_raw_stack_item(item, network);
            RawStackRow {
                field: format!("field{field_index}:"),
                type_name,
                value,
            }
        })
        .collect::<Vec<_>>();

    let field_width = rows.iter().map(|row| row.field.len()).max().unwrap_or(0);
    let type_width = rows
        .iter()
        .map(|row| row.type_name.len())
        .max()
        .unwrap_or(0);

    for row in rows {
        println!(
            "{:<field_width$} {:<type_width$} = {}",
            row.field, row.type_name, row.value
        );
    }
}

fn format_raw_stack_item(item: &TupleItem, network: &Network) -> (&'static str, String) {
    match item {
        TupleItem::Null => ("null", "null".dimmed().to_string()),
        TupleItem::Int(value) => ("int", value.to_string().yellow().to_string()),
        TupleItem::Nan => ("nan", "NaN".dimmed().to_string()),
        TupleItem::Cont(cont) => ("cont", Boc::encode_base64(&cont.code)),
        TupleItem::Cell(cell) => ("cell", format_raw_cell_like(cell, network)),
        TupleItem::Slice(cell) => ("slice", format_raw_cell_like(cell, network)),
        TupleItem::Builder(cell) => ("builder", Boc::encode_base64(cell)),
        TupleItem::Tuple(tuple) => ("tuple", format_raw_stack_tuple(tuple, network)),
    }
}

fn format_raw_cell_like(cell: &Cell, network: &Network) -> String {
    if let Some(value) = format_raw_cell_address_like(cell, network) {
        return value;
    }

    Boc::encode_base64(cell)
}

fn format_raw_cell_address_like(cell: &Cell, network: &Network) -> Option<String> {
    if cell.reference_count() != 0 {
        return None;
    }

    match cell.bit_len() {
        2 => matches!(cell.parse::<AnyAddr>().ok()?, AnyAddr::None)
            .then(|| "addr_none".dimmed().to_string()),
        267 => cell
            .parse::<IntAddr>()
            .ok()
            .map(|address| format_int_address(&address, network).cyan().to_string()),
        _ => None,
    }
}

fn format_raw_stack_tuple(tuple: &Tuple, network: &Network) -> String {
    let items = tuple
        .iter()
        .map(|item| {
            let (type_name, value) = format_raw_stack_item(item, network);
            format!("{type_name} = {value}")
        })
        .collect::<Vec<_>>()
        .join(", ");

    format!("[{items}]")
}

#[derive(Debug, Clone)]
enum GetMethodExitErrorInfo {
    MethodNotFound,
    Abi(crate::formatter::AbiExitCodeInfo),
    Unknown,
}

fn get_method_exit_error_info(exit_code: i32, abi: Option<&ContractABI>) -> GetMethodExitErrorInfo {
    if exit_code == 11 {
        return GetMethodExitErrorInfo::MethodNotFound;
    }

    FormatterContext::find_custom_exit_code_info(exit_code, abi)
        .map_or(GetMethodExitErrorInfo::Unknown, GetMethodExitErrorInfo::Abi)
}

fn get_method_exit_error_plain(
    method: &str,
    exit_code: i32,
    info: &GetMethodExitErrorInfo,
) -> String {
    match info {
        GetMethodExitErrorInfo::MethodNotFound => {
            format!("Get method {method} not found (exit code {exit_code})")
        }
        GetMethodExitErrorInfo::Abi(info) if info.description != info.symbolic_name => format!(
            "Get method {method} failed: {}: {} (exit code {exit_code})",
            info.symbolic_name, info.description
        ),
        GetMethodExitErrorInfo::Abi(info) => format!(
            "Get method {method} failed: {} (exit code {exit_code})",
            info.symbolic_name
        ),
        GetMethodExitErrorInfo::Unknown => {
            format!("Get method {method} failed (exit code {exit_code})")
        }
    }
}

fn get_method_exit_error_json(
    method: &str,
    exit_code: i32,
    abi: Option<&ContractABI>,
) -> serde_json::Value {
    let info = get_method_exit_error_info(exit_code, abi);
    let mut error = serde_json::json!({
        "message": get_method_exit_error_plain(method, exit_code, &info),
    });

    if let GetMethodExitErrorInfo::Abi(info) = info {
        let description = (info.description != info.symbolic_name).then_some(info.description);
        error["name"] = serde_json::Value::String(info.symbolic_name);
        if let Some(description) = description {
            error["description"] = serde_json::Value::String(description);
        }
    }

    error
}

fn get_method_exit_error(method: &str, exit_code: i32, abi: Option<&ContractABI>) -> String {
    match get_method_exit_error_info(exit_code, abi) {
        GetMethodExitErrorInfo::MethodNotFound => format!(
            "Get method {} not found (exit code {exit_code})",
            method.yellow()
        ),
        GetMethodExitErrorInfo::Abi(info) => {
            let exit_name = info.symbolic_name.yellow();
            let exit_info = if info.description == info.symbolic_name {
                exit_name.to_string()
            } else {
                format!("{}: {}", exit_name, info.description.dimmed())
            };

            format!(
                "Get method {} failed: {exit_info} (exit code {exit_code})",
                method.yellow()
            )
        }
        GetMethodExitErrorInfo::Unknown => format!(
            "Get method {} failed (exit code {exit_code})",
            method.yellow()
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn custom_network_uses_mainnet_address_format_for_decoded_results() {
        let fields = vec![
            ("mainnet".to_owned(), RenderedValue::leaf("mainnet-address")),
            ("testnet".to_owned(), RenderedValue::leaf("testnet-address")),
        ];

        assert_eq!(
            rendered_address_value("raw-address", &fields, &Network::Custom(Arc::from("mock"))),
            "mainnet-address"
        );
        assert!(matches!(
            pretty_address_format(&Network::Custom(Arc::from("mock"))),
            PrettyAddressFormat::Mainnet
        ));
    }

    #[test]
    fn invalid_bool_leaf_stays_string_in_json() {
        assert_eq!(
            leaf_value_to_json("not a TVM int", Some("bool")),
            serde_json::Value::String("not a TVM int".to_owned())
        );
    }
}
