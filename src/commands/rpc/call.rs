use super::{
    LocalContractMatch, find_local_contract_match, format_int_address, format_std_address,
    load_rpc_config, resolve_rpc_network,
};
use crate::commands::abi_args::parse_abi_parameters;
use crate::commands::common::error_fmt;
use crate::context::code_lookup_hash;
use acton_config::color::OwoColorize;
use acton_config::config::ActonConfig;
use acton_debug::{
    PrettyAddressFormat, PrettyRenderOptions, RenderedValue, render_abi_tuple_as_tolk_type,
};
use anyhow::{Context, anyhow};
use log::warn;
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
        .get_account_info(None, &address.to_string())
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
    let get_method = match abi {
        Some(abi) => Some(resolve_get_method(abi, method)?),
        None if args.is_empty() => None,
        None => anyhow::bail!(
            "Cannot parse get-method arguments without ABI for remote contract {}",
            address.to_string().yellow()
        ),
    };

    let stack = if let (Some(abi), Some(get_method)) = (abi, get_method) {
        parse_abi_parameters(abi, &get_method.parameters, args)?
    } else {
        Tuple::empty()
    };
    let stack_json = legacy_stack_to_json(&stack).context("Failed to encode get-method stack")?;

    let result = client
        .run_get_method(&address.to_string(), method, &stack_json)
        .with_context(|| {
            format!("Failed to run get method {method} on {address} from {network}")
        })?;
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
        let output = serde_json::json!({
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

        if result.exit_code != 0 {
            println!(
                "\n{} {}",
                "Error:".red(),
                get_method_exit_error(method, result.exit_code)
            );
            let _ = stdout().flush();
            let _ = stderr().flush();
            process::exit(1);
        }
    }

    if result.exit_code != 0 {
        anyhow::bail!("{}", get_method_exit_error(method, result.exit_code));
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

fn resolve_get_method<'a>(abi: &'a ContractABI, method: &str) -> anyhow::Result<&'a ABIGetMethod> {
    abi.get_methods
        .iter()
        .find(|get_method| get_method.name == method)
        .ok_or_else(|| {
            let available = abi
                .get_methods
                .iter()
                .map(|method| format!(" {}", method.name.yellow()))
                .collect::<Vec<_>>()
                .join("\n");
            anyhow!(
                "Get method {} not found in ABI for {}\nAvailable get methods:\n{}",
                method.yellow(),
                abi.contract_name.green(),
                if available.is_empty() {
                    " none".dimmed().to_string()
                } else {
                    available
                }
            )
        })
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

    let rendered = render_abi_tuple_as_tolk_type(abi, tuple, return_ty_idx);
    let options = PrettyRenderOptions {
        address_format: pretty_address_format(network),
        address_labels: Default::default(),
        colorize: false,
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
        return serde_json::Value::Bool(value == "true");
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
    let field_name = match network {
        Network::Mainnet => "mainnet",
        Network::Testnet | Network::Localnet | Network::Custom(_) => "testnet",
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

const fn pretty_address_format(network: &Network) -> PrettyAddressFormat {
    match network {
        Network::Mainnet => PrettyAddressFormat::Mainnet,
        Network::Testnet | Network::Localnet | Network::Custom(_) => PrettyAddressFormat::Testnet,
    }
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

fn get_method_exit_error(method: &str, exit_code: i32) -> String {
    if exit_code == 11 {
        return format!(
            "Get method {} not found (exit code {exit_code})",
            method.yellow()
        );
    }

    format!("Get method {method} exited with code {exit_code}")
}

fn format_get_method_signature(abi: &ContractABI, method: &ABIGetMethod) -> String {
    let params = method
        .parameters
        .iter()
        .map(|param| format!("{}: {}", param.name, abi.render_type(param.ty_idx)))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "{}({}): {}",
        method.name,
        params,
        abi.render_type(method.return_ty_idx)
    )
}
