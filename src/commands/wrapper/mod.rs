use crate::commands::common::error_fmt;
use acton_config::color::OwoColorize;
use acton_config::config::{ActonConfig, project_root};
use anyhow::{Context, anyhow};
use heck::ToLowerCamelCase;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;
use tolk_compiler::abi::{ABIGetMethod, ABIResolvedStruct, ContractABI};
use tolk_compiler::source_map::Declaration;
use tolk_compiler::{CompilerResult, SourceMap};

const TYPESCRIPT_WRAPPER_PACKAGE: &str = "gen-typescript-from-tolk-dev@0.2.4";
const DEFAULT_TOLK_WRAPPER_DIR: &str = "wrappers";
const DEFAULT_TYPESCRIPT_WRAPPER_DIR: &str = "wrappers-ts";

struct WrapperModel {
    project_root: PathBuf,
    contract_id: String,
    contract_name: String,
    abi: ContractABI,
    code_boc64: String,
    storage: Option<ABIResolvedStruct>,
    incoming_messages: Vec<ABIResolvedStruct>,
    storage_path: Option<PathBuf>,
    message_paths: Vec<PathBuf>,
    wrapper_path: PathBuf,
    test_path: PathBuf,
    mappings: Option<BTreeMap<String, String>>,
    format_options: tolk_fmt::FormatOptions,
}

#[derive(Serialize)]
struct TypescriptGeneratorAbi {
    #[serde(flatten)]
    abi: ContractABI,
    #[serde(rename = "codeBoc64")]
    code_boc64: String,
}

fn build_model(
    config: &ActonConfig,
    contract_id: &str,
    wrapper_output: Option<String>,
    wrapper_output_dir: Option<String>,
    test_output: Option<String>,
    test_output_dir: Option<String>,
    generate_typescript: bool,
) -> anyhow::Result<WrapperModel> {
    let format_options = {
        let fmt_settings = config.fmt.as_ref();
        let width = fmt_settings.and_then(|s| s.width).unwrap_or(100);
        let separate_import_groups = fmt_settings
            .and_then(|s| s.separate_import_groups)
            .unwrap_or(false);
        tolk_fmt::FormatOptions {
            width,
            separate_import_groups,
        }
    };
    let project_root = project_root().to_path_buf();

    let contract_config = config
        .get_contract(contract_id)
        .ok_or_else(|| anyhow!(error_fmt::contract_not_found(config, contract_id)))?;

    let contract_path = contract_config.absolute_source_path(&project_root);

    if !contract_path.exists() {
        anyhow::bail!(
            "Contract file for {} not found: {} (specified in Acton.toml as {})",
            contract_id.yellow(),
            contract_path.display().to_string().yellow(),
            contract_config.src.yellow()
        );
    }

    let mappings = config.mappings();
    let compiler = tolk_compiler::Compiler::new(2).with_mappings(&mappings);
    let (abi, code_boc64, source_map) = match compiler.compile(&contract_path, false) {
        CompilerResult::Success(result) => (
            result.abi.ok_or_else(|| {
                anyhow!("Compiler did not produce ABI for {}", contract_id.yellow())
            })?,
            result.code_boc64,
            result.source_map.ok_or_else(|| {
                anyhow!(
                    "Compiler did not produce symbol types for {}",
                    contract_id.yellow()
                )
            })?,
        ),
        CompilerResult::Error(error) => {
            anyhow::bail!(
                "Failed to compile contract {} for wrapper generation: {}",
                contract_id.yellow(),
                error.message
            );
        }
    };

    let file_stem = contract_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(contract_id);

    let contract_name = to_pascal_case(file_stem);
    let configured_tolk_output_dir = config.tolk_wrapper_output_dir().map(ToOwned::to_owned);
    let configured_typescript_output_dir = config
        .typescript_wrapper_output_dir()
        .map(ToOwned::to_owned);
    let configured_tolk_test_output_dir =
        config.tolk_wrapper_test_output_dir().map(ToOwned::to_owned);
    let mapped_wrapper_output_dir = mappings
        .as_ref()
        .and_then(|mappings| mappings.get("@wrappers").cloned());
    let storage = abi.resolve_storage_struct()?;
    let incoming_messages = abi.resolve_incoming_message_structs()?;
    let storage_path = storage
        .iter()
        .find_map(|storage| find_type_path(&source_map, &storage.name));
    let message_paths = incoming_messages
        .iter()
        .filter_map(|message| find_type_path(&source_map, &message.name))
        .collect::<BTreeSet<_>>();

    let wrapper_path = resolve_wrapper_path(
        &project_root,
        &contract_name,
        wrapper_output,
        wrapper_output_dir,
        configured_tolk_output_dir,
        configured_typescript_output_dir,
        mapped_wrapper_output_dir,
        generate_typescript,
    );
    let test_path = resolve_test_path(
        &project_root,
        contract_id,
        test_output,
        test_output_dir,
        configured_tolk_test_output_dir,
    );

    let message_paths = message_paths.into_iter().collect();

    Ok(WrapperModel {
        project_root,
        contract_id: contract_id.to_owned(),
        contract_name,
        abi,
        code_boc64,
        storage,
        incoming_messages,
        storage_path,
        message_paths,
        wrapper_path,
        test_path,
        mappings,
        format_options,
    })
}

fn format_generated_tolk(
    model: &WrapperModel,
    raw: String,
    output_path: &Path,
    artifact_label: &str,
) -> String {
    match tolk_fmt::format_source(&raw, model.format_options) {
        Ok(formatted) => formatted,
        Err(err) => {
            eprintln!(
                "{} Failed to format generated {} {}: {}. Writing unformatted output.",
                "Error:".red().bold(),
                artifact_label,
                output_path.display().to_string().yellow(),
                err
            );
            raw
        }
    }
}

fn generated_wrapper_header(contract_name: &str) -> String {
    format!(
        "// Auto-generated wrapper for contract '{contract_name}'
//
// This file is automatically generated by 'acton wrapper'
// Do not edit manually — changes will be overwritten\n\n"
    )
}

pub fn wrapper_cmd(
    contract_id: &str,
    wrapper_output: Option<String>,
    wrapper_output_dir: Option<String>,
    test_output: Option<String>,
    test_output_dir: Option<String>,
    generate_test_stub: bool,
    generate_typescript: bool,
) -> anyhow::Result<()> {
    let config = ActonConfig::load().map_err(|e| anyhow!("Failed to load Acton.toml: {e}"))?;

    let explicit_test_request = generate_test_stub
        || has_non_empty_path(test_output.as_deref())
        || has_non_empty_path(test_output_dir.as_deref());

    if generate_typescript && explicit_test_request {
        anyhow::bail!(
            "`acton wrapper --ts` does not support `--test`, `--test-output`, or `--test-output-dir`"
        );
    }

    let generate_test_stub =
        !generate_typescript && (explicit_test_request || config.tolk_wrapper_generate_test());

    let model = build_model(
        &config,
        contract_id,
        wrapper_output,
        wrapper_output_dir,
        test_output,
        test_output_dir,
        generate_typescript,
    )?;

    if let Some(parent) = model.wrapper_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| anyhow!("Failed to create directory {}: {}", parent.display(), e))?;
    }

    if generate_typescript {
        let wrapper_code = generate_typescript_wrapper(&model)?;
        fs::write(&model.wrapper_path, wrapper_code)
            .map_err(|e| anyhow!("Failed to write wrapper file: {e}"))?;
    } else {
        let wrapper_code = generate_wrapper(&model);
        let wrapper_code =
            format_generated_tolk(&model, wrapper_code, &model.wrapper_path, "wrapper");
        let wrapper_code = format!(
            "{}{}",
            generated_wrapper_header(&model.contract_name),
            wrapper_code
        );

        fs::write(&model.wrapper_path, wrapper_code)
            .map_err(|e| anyhow!("Failed to write wrapper file: {e}"))?;

        if generate_test_stub {
            if let Some(parent) = model.test_path.parent() {
                fs::create_dir_all(parent).map_err(|e| {
                    anyhow!("Failed to create directory {}: {}", parent.display(), e)
                })?;
            }

            let test_code = generate_test(&model);
            let test_code = format_generated_tolk(&model, test_code, &model.test_path, "test stub");
            fs::write(&model.test_path, test_code)
                .map_err(|e| anyhow!("Failed to write test file: {e}"))?;
        }
    }

    let wrapper_relative = model
        .wrapper_path
        .strip_prefix(&model.project_root)
        .unwrap_or(&model.wrapper_path)
        .to_string_lossy();

    let test_relative = model
        .test_path
        .strip_prefix(&model.project_root)
        .unwrap_or(&model.test_path)
        .to_string_lossy();

    println!("   {} {}", "Generated".green().bold(), wrapper_relative);

    if generate_test_stub {
        println!("   {} {}", "Generated".green().bold(), test_relative);
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn resolve_wrapper_path(
    project_root: &Path,
    contract_name: &str,
    wrapper_output: Option<String>,
    wrapper_output_dir: Option<String>,
    configured_tolk_output_dir: Option<String>,
    configured_ts_output_dir: Option<String>,
    mapped_wrapper_output_dir: Option<String>,
    generate_typescript: bool,
) -> PathBuf {
    if let Some(wrapper_output) = non_empty_path(wrapper_output) {
        return PathBuf::from(wrapper_output);
    }

    let file_name = wrapper_file_name(contract_name, generate_typescript);

    if let Some(wrapper_output_dir) = non_empty_path(wrapper_output_dir) {
        return PathBuf::from(wrapper_output_dir).join(&file_name);
    }

    if generate_typescript {
        if let Some(configured_ts_output_dir) = non_empty_path(configured_ts_output_dir) {
            return resolve_project_config_path(project_root, &configured_ts_output_dir)
                .join(&file_name);
        }

        return project_root
            .join(DEFAULT_TYPESCRIPT_WRAPPER_DIR)
            .join(&file_name);
    }

    if let Some(configured_tolk_output_dir) = non_empty_path(configured_tolk_output_dir) {
        return resolve_project_config_path(project_root, &configured_tolk_output_dir)
            .join(&file_name);
    }

    if let Some(mapped_wrapper_output_dir) = non_empty_path(mapped_wrapper_output_dir) {
        return resolve_project_config_path(project_root, &mapped_wrapper_output_dir)
            .join(&file_name);
    }

    project_root.join(DEFAULT_TOLK_WRAPPER_DIR).join(&file_name)
}

fn resolve_test_path(
    project_root: &Path,
    contract_id: &str,
    test_output: Option<String>,
    test_output_dir: Option<String>,
    configured_tolk_test_output_dir: Option<String>,
) -> PathBuf {
    if let Some(test_output) = non_empty_path(test_output) {
        return PathBuf::from(test_output);
    }

    let file_name = format!("{contract_id}.test.tolk");

    if let Some(test_output_dir) = non_empty_path(test_output_dir) {
        return PathBuf::from(test_output_dir).join(&file_name);
    }

    if let Some(configured_tolk_test_output_dir) = non_empty_path(configured_tolk_test_output_dir) {
        return resolve_project_config_path(project_root, &configured_tolk_test_output_dir)
            .join(&file_name);
    }

    project_root.join("tests").join(&file_name)
}

fn wrapper_file_name(contract_name: &str, generate_typescript: bool) -> String {
    let extension = if generate_typescript {
        "gen.ts"
    } else {
        "gen.tolk"
    };
    format!("{contract_name}.{extension}")
}

fn non_empty_path(path: Option<String>) -> Option<String> {
    path.and_then(|path| {
        if path.trim().is_empty() {
            None
        } else {
            Some(path)
        }
    })
}

fn has_non_empty_path(path: Option<&str>) -> bool {
    path.is_some_and(|path| !path.trim().is_empty())
}

fn resolve_project_config_path(project_root: &Path, path: &str) -> PathBuf {
    let path = Path::new(path);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        project_root.join(path)
    }
}

fn generate_typescript_wrapper(model: &WrapperModel) -> anyhow::Result<String> {
    let abi_json = serialize_typescript_abi(model)?;
    let npm_cache_dir =
        TempDir::new().context("Failed to create a temporary npm cache directory")?;

    let output = Command::new("npx")
        .env("npm_config_cache", npm_cache_dir.path())
        .env("npm_config_update_notifier", "false")
        .arg("--yes")
        .arg(TYPESCRIPT_WRAPPER_PACKAGE)
        .arg(abi_json)
        .output()
        .with_context(|| {
            format!(
                "Failed to execute `npx {TYPESCRIPT_WRAPPER_PACKAGE}`. Ensure Node.js/npm is installed and `npx` is available in PATH."
            )
        })?;

    if !output.status.success() {
        let code = output.status.code().unwrap_or(1);
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        let details = if !stderr.is_empty() {
            stderr
        } else if !stdout.is_empty() {
            stdout
        } else {
            "no output".to_owned()
        };

        anyhow::bail!("`npx {TYPESCRIPT_WRAPPER_PACKAGE}` failed with exit code {code}: {details}");
    }

    String::from_utf8(output.stdout)
        .context("TypeScript wrapper generator emitted non-UTF-8 output")
}

fn serialize_typescript_abi(model: &WrapperModel) -> anyhow::Result<String> {
    let mut abi = model.abi.clone();
    if abi.contract_name.is_empty() {
        abi.contract_name.clone_from(&model.contract_name);
    }

    serde_json::to_string(&TypescriptGeneratorAbi {
        abi,
        code_boc64: model.code_boc64.clone(),
    })
    .context("Failed to encode ABI JSON for TypeScript wrapper generation")
}

fn find_type_path(source_map: &SourceMap, type_name: &str) -> Option<PathBuf> {
    source_map.declarations().iter().find_map(|declaration| {
        let Declaration::Struct(struct_decl) = declaration else {
            return None;
        };

        if struct_decl.name != type_name {
            return None;
        }

        source_map
            .resolve_file_full_path(struct_decl.ident_loc.file_id())
            .map(PathBuf::from)
    })
}

fn to_pascal_case(s: &str) -> String {
    let mut result = String::new();
    let mut capitalize_next = true;

    for ch in s.chars() {
        if ch == '_' || ch == '-' {
            capitalize_next = true;
        } else if capitalize_next {
            result.push(ch.to_uppercase().next().unwrap_or(ch));
            capitalize_next = false;
        } else {
            result.push(ch);
        }
    }

    if result.is_empty() {
        result.push_str(s);
    }

    result
}

fn generate_wrapper(model: &WrapperModel) -> String {
    let proot = &model.project_root;
    let root = &model.wrapper_path;
    let contract = &model.contract_name;
    let mappings = &model.mappings;

    let mut code = String::new();

    code.push_str(&import_stdlib("build"));
    code.push_str(&import_stdlib("emulation/network"));
    code.push_str(&import_stdlib("testing/assert"));

    if let Some(storage_path) = &model.storage_path {
        let storage_import = get_import_path(proot, root, storage_path, mappings.as_ref());
        code.push_str(&gen_import_path(storage_import));
    }

    for messages_path in &model.message_paths {
        if Some(messages_path) == model.storage_path.as_ref() {
            // don't add duplicate import
            continue;
        }

        let types_import = get_import_path(proot, root, messages_path, mappings.as_ref());
        code.push_str(&gen_import_path(types_import));
    }

    code.push('\n');

    if let (Some(storage), Some(storage_path)) = (&model.storage, &model.storage_path) {
        let import_path = get_import_path(proot, root, storage_path, mappings.as_ref());
        let display = import_path.display().to_string();
        let display = display.trim_start_matches("./").trim_end_matches(".tolk");
        let _ = writeln!(
            code,
            "/// Storage `{}` is defined in `{display}`",
            storage.name
        );
    }
    let _ = writeln!(code, "struct {contract} {{");
    code.push_str("    address: address\n");
    code.push_str("    stateInit: ContractState? = null\n");
    code.push_str("}\n\n");

    if let Some(storage) = &model.storage {
        code.push_str(&generate_from_storage(
            contract,
            &model.contract_id,
            &storage.name,
        ));
    } else {
        code.push_str(&generate_empty_from_storage(contract, &model.contract_id));
    }

    code.push('\n');
    code.push_str(&generate_from_address(contract));
    code.push('\n');
    code.push_str(&generate_deploy(contract));
    code.push('\n');

    for message in &model.incoming_messages {
        code.push_str(&generate_send_method(contract, message));
        code.push('\n');
    }

    code.push_str(&generate_send_any_method(contract));
    code.push('\n');

    for get_method in &model.abi.get_methods {
        code.push_str(&generate_get_method(contract, get_method));
        code.push('\n');
    }

    format!("{}\n", code.trim())
}

fn generate_from_storage(
    contract_name: &str,
    contract_build_name: &str,
    storage_name: &str,
) -> String {
    let mut code = String::new();

    code.push_str("/// Creates a contract wrapper instance from the storage data\n");
    let _ = writeln!(
        code,
        "fun {contract_name}.fromStorage(storage: {storage_name}, toShard: AddressShardingOptions? = null): {contract_name} {{"
    );
    code.push_str("    val stateInit = ContractState {\n");
    let _ = writeln!(code, "        code: build(\"{contract_build_name}\"),");
    code.push_str("        data: storage.toCell(),\n");
    code.push_str("    };\n");
    code.push_str(
        "    val address = AutoDeployAddress { stateInit, toShard }.calculateAddress();\n",
    );
    let _ = writeln!(code, "    return {contract_name} {{ address, stateInit }}");
    code.push_str("}\n");

    code
}

fn generate_from_address(contract_name: &str) -> String {
    let mut code = String::new();

    code.push_str("/// Creates a contract wrapper instance from the address\n");
    let _ = writeln!(
        code,
        "fun {contract_name}.fromAddress(address: address): {contract_name} {{"
    );
    let _ = writeln!(code, "    return {contract_name} {{ address }}");
    code.push_str("}\n");

    code
}

fn generate_empty_from_storage(contract_name: &str, contract_build_name: &str) -> String {
    let mut code = String::new();

    code.push_str("/// Creates a contract wrapper instance from the storage data\n");
    let _ = writeln!(
        code,
        "fun {contract_name}.fromStorage(toShard: AddressShardingOptions? = null): {contract_name} {{"
    );
    code.push_str("    val stateInit = ContractState {\n");
    let _ = writeln!(code, "        code: build(\"{contract_build_name}\"),");
    code.push_str("        data: createEmptyCell(),\n");
    code.push_str("    };\n");
    code.push_str(
        "    val address = AutoDeployAddress { stateInit, toShard }.calculateAddress();\n",
    );
    let _ = writeln!(code, "    return {contract_name} {{ address, stateInit }}");
    code.push_str("}\n");

    code
}

fn generate_deploy(contract_name: &str) -> String {
    let mut code = String::new();

    code.push_str("/// Deploys the contract to the blockchain\n");
    let _ = writeln!(
        code,
        "fun {contract_name}.deploy(self, from: address, config: SendParams = {{}}): SendResultList {{"
    );
    code.push_str("    if (self.stateInit == null) {\n");
    code.push_str("        Assert.fail(\"Cannot deploy a contract created with 'fromAddress' because it lacks state init for deployment\");\n");
    code.push_str("    }\n");
    code.push_str("    val genericMsg = createMessage({\n");
    code.push_str("        bounce: config.bounce,\n");
    code.push_str("        value: config.value,\n");
    code.push_str("        dest: {\n");
    code.push_str("            stateInit: self.stateInit,\n");
    code.push_str("        },\n");
    code.push_str("    });\n");
    code.push_str("    return net.send(from, genericMsg)\n");
    code.push_str("}\n");

    code
}

fn generate_send_method(contract_name: &str, message_type: &ABIResolvedStruct) -> String {
    let mut code = String::new();
    let method_name = format!("send{}", message_type.name);

    let fields: Vec<_> = message_type.fields.iter().collect();

    let params = fields
        .iter()
        .map(|f| {
            let type_name = f.ty.render_param_type();
            let name = normalize_param_name(&f.name);
            format!("{name}: {type_name}")
        })
        .collect::<Vec<_>>()
        .join(", ");

    let params_str = if params.is_empty() {
        String::new()
    } else {
        format!("{params}, ")
    };

    let _ = writeln!(
        code,
        "fun {contract_name}.{method_name}(self, from: address, {params_str}config: SendParams = {{}}): SendResultList {{"
    );
    code.push_str("    val genericMsg = createMessage({\n");
    code.push_str("        bounce: config.bounce,\n");
    code.push_str("        value: config.value,\n");
    code.push_str("        dest: self.address,\n");

    if fields.is_empty() {
        let _ = writeln!(code, "        body: {} {{}},", message_type.name);
    } else {
        let _ = writeln!(code, "        body: {} {{", message_type.name);
        for field in &fields {
            let param_name = normalize_param_name(&field.name);

            if field.ty.is_typed_cell() {
                let _ = writeln!(code, "            {}: {}.toCell(),", field.name, param_name);
            } else if field.name == param_name {
                let _ = writeln!(code, "            {},", field.name);
            } else {
                let _ = writeln!(code, "            {}: {},", field.name, param_name);
            }
        }
        code.push_str("        },\n");
    }

    code.push_str("    });\n");
    code.push_str("    return net.send(from, genericMsg)\n");
    code.push_str("}\n");

    code
}

fn normalize_param_name(name: &str) -> String {
    if name == "from" || name == "config" {
        format!("{name}_")
    } else {
        name.to_owned()
    }
}

fn generate_send_any_method(contract_name: &str) -> String {
    let mut code = String::new();

    code.push_str("/// Send message to the contract with a custom body cell\n");
    let _ = writeln!(
        code,
        "fun {contract_name}.sendAny(self, from: address, body: cell, config: SendParams = {{}}): SendResultList {{"
    );
    code.push_str("    val genericMsg = createMessage({\n");
    code.push_str("        bounce: config.bounce,\n");
    code.push_str("        value: config.value,\n");
    code.push_str("        dest: self.address,\n");
    code.push_str("        body,\n");
    code.push_str("    });\n");
    code.push_str("    return net.send(from, genericMsg)\n");
    code.push_str("}\n");

    code
}

fn generate_get_method(contract_name: &str, get_method: &ABIGetMethod) -> String {
    let mut code = String::new();
    let method_name = normalize_get_method_name(&get_method.name);
    let tvm_method_name = &get_method.name;
    let params = get_method
        .parameters
        .iter()
        .map(|p| {
            let type_name = p.ty.render_param_type();
            let param_name = normalize_get_param_name(&p.name);
            format!("{param_name}: {type_name}")
        })
        .collect::<Vec<_>>()
        .join(", ");

    let args = get_method
        .parameters
        .iter()
        .map(|p| {
            let param_name = normalize_get_param_name(&p.name);
            if p.ty.is_typed_cell() {
                format!("{param_name}.toCell()")
            } else {
                param_name
            }
        })
        .collect::<Vec<_>>();

    let return_type = get_method.return_ty.render_type();

    if params.is_empty() {
        let _ = writeln!(
            code,
            "fun {contract_name}.{method_name}(self): {return_type} {{"
        );
    } else {
        let _ = writeln!(
            code,
            "fun {contract_name}.{method_name}(self, {params}): {return_type} {{"
        );
    }

    if args.is_empty() {
        let _ = writeln!(
            code,
            "    return net.runGetMethod(self.address, \"{tvm_method_name}\")"
        );
    } else {
        let args = args.join(", ");

        let _ = writeln!(
            code,
            "    return net.runGetMethod(self.address, \"{tvm_method_name}\", [{args}])"
        );
    }

    code.push_str("}\n");

    code
}

fn normalize_get_method_name(name: &str) -> String {
    name.to_lower_camel_case()
}

fn normalize_get_param_name(name: &str) -> String {
    let normalized = name.to_lower_camel_case();
    if normalized == "from" || normalized == "config" {
        format!("{normalized}_")
    } else {
        normalized
    }
}

fn generate_test(model: &WrapperModel) -> String {
    let proot = &model.project_root;
    let root = &model.test_path;
    let contract = &model.contract_name;
    let mappings = &model.mappings;

    let mut code = String::new();

    code.push_str("import \"@stdlib/gas-payments\"\n");
    code.push_str(&import_stdlib("emulation/network"));
    code.push_str(&import_stdlib("emulation/testing"));
    code.push_str(&import_stdlib("testing/expect"));

    for messages_path in &model.message_paths {
        let types_import = get_import_path(proot, root, messages_path, mappings.as_ref());
        code.push_str(&gen_import_path(types_import));
    }

    let wrapper_import = get_import_path(proot, root, &model.wrapper_path, mappings.as_ref());
    code.push_str(&gen_import_path(wrapper_import));
    code.push('\n');

    code.push_str(&generate_example_test(contract));
    code.push('\n');

    code.push_str(&generate_setup_test(
        contract,
        &model.abi,
        model.storage.as_ref(),
    ));

    format!("{}\n", code.trim())
}

fn import_stdlib(path: &str) -> String {
    gen_import_path(PathBuf::from("@acton").join(path))
}

fn gen_import_path(path: PathBuf) -> String {
    let path = path.display().to_string();
    format!(
        "import \"{}\"\n",
        path.trim_start_matches("./").trim_end_matches(".tolk")
    )
}

fn get_relative_import(project_root: &Path, where_: &Path, what: &Path) -> PathBuf {
    let Some(where_dir) = where_.parent() else {
        return what.to_path_buf();
    };
    let what_abs_path = project_root.join(what);
    let where_abs_dir = project_root.join(where_dir);

    let relative_path = pathdiff::diff_paths(&what_abs_path, where_abs_dir);
    relative_path.unwrap_or(what_abs_path)
}

fn get_import_path(
    project_root: &Path,
    where_: &Path,
    what: &Path,
    mappings: Option<&BTreeMap<String, String>>,
) -> PathBuf {
    if let Some(mapped_import) = resolve_mapped_import(project_root, what, mappings) {
        return mapped_import;
    }

    get_relative_import(project_root, where_, what)
}

fn resolve_mapped_import(
    project_root: &Path,
    what: &Path,
    mappings: Option<&BTreeMap<String, String>>,
) -> Option<PathBuf> {
    let mappings = mappings?;
    let what_abs = normalize_abs_path(project_root, what);

    let mut best_match = None;

    for (key, value) in mappings {
        let mapping_abs = normalize_abs_path(project_root, Path::new(value));

        if !what_abs.starts_with(&mapping_abs) {
            continue;
        }

        let Ok(relative_path) = what_abs.strip_prefix(&mapping_abs) else {
            continue;
        };

        let key = if key.starts_with('@') {
            key.clone()
        } else {
            format!("@{key}")
        };

        let mut import_path = PathBuf::from(key);
        if !relative_path.as_os_str().is_empty() {
            import_path = import_path.join(relative_path);
        }

        let score = mapping_abs.components().count();
        if best_match
            .as_ref()
            .is_none_or(|(best_score, _)| score > *best_score)
        {
            best_match = Some((score, import_path));
        }
    }

    best_match.map(|(_, path)| path)
}

fn normalize_abs_path(project_root: &Path, path: &Path) -> PathBuf {
    let abs_path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        project_root.join(path)
    };

    dunce::canonicalize(&abs_path).unwrap_or(abs_path)
}

fn generate_setup_test(
    contract_name: &str,
    abi: &ContractABI,
    storage: Option<&ABIResolvedStruct>,
) -> String {
    let mut code = String::new();

    code.push_str(
        "/// Initializes the test environment, creating a fresh instance of the contract.\n",
    );
    code.push_str("/// Returns the contract wrapper and two treasury accounts (`deployer` and `not_deployer`).\n");
    let _ = writeln!(
        code,
        "fun setupTest(): ({contract_name}, Treasury, Treasury) {{"
    );

    code.push_str("    // Create a treasury account for deployment (typically the owner)\n");
    code.push_str("    val deployer = testing.treasury(\"deployer\");\n");
    code.push_str(
        "    // Create another treasury account for testing interactions from other users\n",
    );
    code.push_str("    val not_deployer = testing.treasury(\"not_deployer\");\n");
    code.push('\n');
    code.push_str("    // Initialize and deploy the contract with default values\n");

    if let Some(storage) = storage {
        let _ = write!(code, "    val contract = {contract_name}.fromStorage({{");

        let storage_fields = storage
            .fields
            .iter()
            .map(|f| {
                if let Some(default_value) = f.ty.typed_cell_payload_default_value(abi) {
                    format!(" {}: {default_value}.toCell()", f.name)
                } else {
                    format!(" {}: {}", f.name, f.ty.default_value(abi))
                }
            })
            .collect::<Vec<_>>()
            .join(",");

        code.push_str(&storage_fields);
        code.push_str(" });\n");
    } else {
        let _ = writeln!(code, "    val contract = {contract_name}.fromStorage();");
    }

    code.push_str("    val res = contract.deploy(deployer.address, { value: ton(\"1\") });\n");
    code.push_str("    expect(res).toHaveSuccessfulDeploy({ to: contract.address });\n");
    code.push('\n');
    code.push_str("    return (contract, deployer, not_deployer)\n");
    code.push_str("}\n");

    code
}

fn generate_example_test(_contract_name: &str) -> String {
    let mut code = String::new();

    code.push_str("/// Example test case demonstrating the basic flow\n");
    code.push_str("get fun `test basic flow`() {\n");
    code.push_str("    val (contract, deployer, not_deployer) = setupTest();\n");
    code.push('\n');
    code.push_str("    // TODO: Implement your test logic here\n");
    code.push_str("    // Example:\n");
    code.push_str("    // val res = contract.sendMsg(deployer.address, ...);\n");
    code.push_str("    // expect(res).toHaveTransaction({ ... });\n");
    code.push_str("}\n");

    code
}
