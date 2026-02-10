use crate::commands::common::error_fmt;
use crate::file_build_cache::FileBuildCache;
use crate::stdlib;
use acton_config::config::{ActonConfig, ContractConfig, ContractDependency, DependencyKind};
use anyhow::anyhow;
use log::debug;
use owo_colors::OwoColorize;
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::time::Instant;
use tempfile::TempDir;
use tycho_types::boc::Boc;

mod dep_graph;

pub fn build_cmd(
    contract_id: Option<String>,
    clear_cache: bool,
    graph_output: Option<String>,
    out_dir: Option<String>,
    show_info: bool,
) -> anyhow::Result<()> {
    stdlib::ensure_latest(Path::new("."))?;

    // Due to global variables, we need to enable debug mode for emulator as early as possible
    // since first compilation WITHOUT debug mode will set debug=false forever
    enable_emulator_debug_mode()?;

    let out_dir = out_dir.unwrap_or_else(|| "build".to_string());

    if !Path::new(&out_dir).exists() {
        fs::create_dir_all(&out_dir)?;
    }

    if clear_cache {
        let mut file_cache = FileBuildCache::new(None)?;
        file_cache.clear()?;
        println!("  {} Cache cleared", "✓".green().bold());
    }

    println!("   {} contracts", "Compiling".green().bold());

    let config = ActonConfig::load()?;

    let contracts = match config.contracts() {
        Some(contracts) => contracts,
        None => {
            println!(
                "No contracts section found in Acton.toml. Add at least one contract.
To add a contract add the following section to Acton.toml:

[contracts.my-contract]
name = \"MyContract\"
src = \"contracts/my-contract.tolk\"
depends = []

See https://i582.github.io/acton/docs/build-system/configuration-reference/#contracts-section for more information"
            );
            return Ok(());
        }
    };

    if contracts.is_empty() {
        println!("No contracts to build.");
        return Ok(());
    }

    if let Some(filter) = &contract_id
        && !contracts.iter().any(|(key, _)| key == filter)
    {
        anyhow::bail!(error_fmt::contract_not_found(&config, filter))
    }

    let mut file_cache = FileBuildCache::new(None)?;
    let mut failure_count = 0;
    let total_start = Instant::now();

    let flatten_contracts = contracts.iter().collect::<Vec<_>>();
    let compilation_order = dep_graph::build_dependency_graph(&flatten_contracts)?;
    debug!("Compilation order: {compilation_order:?}");

    let filtered_compilation_order = if let Some(filter) = &contract_id {
        dep_graph::filter_compilation_order_for_contract(filter, &compilation_order, contracts)?
    } else {
        compilation_order
    };

    debug!("Build next contracts: {filtered_compilation_order:?}");

    if let Some(graph_path) = &graph_output {
        let output_path = if graph_path.is_empty() {
            "deps.svg"
        } else {
            graph_path
        };
        dep_graph::generate_dependency_graph_svg(
            &filtered_compilation_order,
            contracts,
            output_path,
        )?;
    }

    let mut compiled_contracts: HashMap<String, String> = HashMap::new();
    let mut compile_errors = BTreeMap::new();
    let mut build_info = Vec::new();

    for parent_contract in filtered_compilation_order {
        let Some(contract_config) = contracts.get(&parent_contract) else {
            continue;
        };
        let contract_path = &contract_config.src;

        generate_dependency_files(
            &parent_contract,
            contract_config,
            &compiled_contracts,
            &compile_errors,
            &config,
        )?;

        let (code_boc64, code_hash) =
            match process_contract(&mut file_cache, contract_config, contract_path, &config) {
                Ok((code, hash)) => (code, hash),
                Err(err) => {
                    failure_count += 1;
                    compile_errors.insert(parent_contract.clone(), err);
                    continue;
                }
            };

        compiled_contracts.insert(parent_contract.clone(), code_boc64.clone());

        if show_info {
            build_info.push((
                contract_config.name.clone(),
                code_boc64.clone(),
                code_hash.clone(),
            ));
        }

        if let Err(e) = save_build_artifact(&out_dir, &parent_contract, &code_boc64, &code_hash) {
            eprintln!(
                "Warning: Failed to save build artifact file for {}: {}",
                contract_config.name, e
            );
        }

        if let Err(e) = save_boc_file(contract_config, &code_boc64) {
            eprintln!(
                "Warning: Failed to save cached BoC file for {}: {}",
                contract_config.name, e
            );
        }
    }

    let total_elapsed = total_start.elapsed();

    if failure_count == 0 {
        println!("    {} in {:?}", "Finished".green().bold(), total_elapsed);

        if !build_info.is_empty() {
            for (name, code, hash) in build_info {
                println!();
                println!("   {} of {}", "Artifacts".green().bold(), name);
                println!("        {} {}", "Code".cyan(), code.dimmed());
                println!("        {} {}", "Hash".cyan(), hash.dimmed());
            }
        }

        Ok(())
    } else {
        let mut whole_error = String::new();

        for (contract, err) in compile_errors {
            whole_error += color_print::cformat!("In <yellow>{contract}</>:\n\n{err}\n").as_str();
        }

        whole_error.push_str(
            color_print::cformat!(
                "<red>Build failed</> with {} error{}",
                failure_count,
                if failure_count == 1 { "" } else { "s" }
            )
            .as_str(),
        );

        Err(anyhow!(whole_error))
    }
}

fn process_contract(
    file_cache: &mut FileBuildCache,
    contract_config: &ContractConfig,
    contract_path: &String,
    acton_config: &ActonConfig,
) -> anyhow::Result<(String, String)> {
    let (code_boc64, code_hash) = if contract_path.ends_with(".boc") {
        debug!("Loading BoC file: {contract_path}");
        match fs::read(contract_path) {
            Ok(boc_data) => match Boc::decode(&boc_data) {
                Ok(boc) => {
                    let code_boc64 = Boc::encode_base64(&boc);
                    (code_boc64, boc.repr_hash().to_string())
                }
                Err(e) => {
                    anyhow::bail!("Failed to decode BoC file {contract_path}: {e}");
                }
            },
            Err(e) => {
                anyhow::bail!("Failed to read BoC file {contract_path}: {e}");
            }
        }
    } else {
        let cached_result = file_cache.get(contract_path, false, 2, "1.3".to_string());

        if let Some(cached_result) = cached_result {
            debug!("Cache hit, use cached result for '{contract_path}'");
            (cached_result.code_boc64, cached_result.code_hash_hex)
        } else {
            debug!("Cache miss, recompile '{contract_path}'");
            let compile_start = Instant::now();
            println!("   {} {}", "Compiling".green().bold(), contract_config.name);

            let compiler = tolkc::Compiler::new(2).with_mappings(&acton_config.mappings);
            let compilation_result = compiler.compile(Path::new(contract_path), false);
            let compile_time = compile_start.elapsed();

            match compilation_result {
                tolkc::CompilerResult::Success(result) => {
                    if let Err(e) =
                        file_cache.put(contract_path, &result, false, 2, "1.3".to_string())
                    {
                        eprintln!(
                            "Warning: Failed to cache compilation result for {}: {}",
                            contract_config.name, e
                        );
                    }

                    println!("    {} in {:?}", "Finished".green(), compile_time);

                    (result.code_boc64, result.code_hash_hex)
                }
                tolkc::CompilerResult::Error(error) => {
                    anyhow::bail!(error.message);
                }
            }
        }
    };
    Ok((code_boc64, code_hash))
}

fn save_boc_file(contract_config: &ContractConfig, code_boc64: &str) -> anyhow::Result<()> {
    if let Some(output_path) = &contract_config.output {
        if let Some(parent_dir) = Path::new(&output_path).parent()
            && let Err(err) = fs::create_dir_all(parent_dir)
        {
            anyhow::bail!(
                "Failed to create directory for BoC file {}: {}",
                parent_dir.display(),
                err
            );
        }

        let code = Boc::decode_base64(code_boc64)?;
        fs::write(output_path, Boc::encode(code))?;
    }
    Ok(())
}

fn save_build_artifact(
    out_dir: &str,
    contract_key: &str,
    code_boc64: &str,
    code_hash: &str,
) -> anyhow::Result<()> {
    use serde_json::json;

    let json_data = json!({
        "code_boc64": code_boc64,
        "hash": code_hash
    });

    let filename = format!("{contract_key}.json");
    let path = Path::new(out_dir).join(filename);
    fs::write(path, serde_json::to_string_pretty(&json_data)?)?;

    Ok(())
}

pub(crate) fn generate_dependency_files(
    parent_contract: &str,
    config: &ContractConfig,
    compiled_contracts: &HashMap<String, String>, // contract_key -> boc_base64
    failed_contracts: &BTreeMap<String, anyhow::Error>,
    acton_config: &ActonConfig,
) -> anyhow::Result<()> {
    let Some(depends) = &config.depends else {
        return Ok(());
    };
    if depends.is_empty() {
        return Ok(());
    }

    for dep in depends {
        generate_single_dependency_file(
            parent_contract,
            dep,
            compiled_contracts,
            failed_contracts,
            acton_config,
        )?;
    }

    Ok(())
}

fn create_gen_dir<'a>() -> anyhow::Result<&'a Path> {
    let gen_dir = Path::new("gen");
    if !gen_dir.exists() {
        fs::create_dir_all(gen_dir)?;
    }
    Ok(gen_dir)
}

fn generate_single_dependency_file(
    parent_contract: &str,
    dependency: &ContractDependency,
    compiled_contracts: &HashMap<String, String>,
    failed_contracts: &BTreeMap<String, anyhow::Error>,
    acton_config: &ActonConfig,
) -> anyhow::Result<()> {
    let gen_dir = create_gen_dir()?;
    let dependency_contract = dependency.name();

    if failed_contracts.get(dependency_contract).is_some() {
        // contract depends on other contract with compilation error, don't do anything
        return Ok(());
    }

    let boc_base64 = compiled_contracts.get(dependency_contract).ok_or_else(|| {
        anyhow!(
            "[INTERNAL ERROR] Dependency '{dependency_contract}' must be compiled before '{parent_contract}'"
        )
    })?;

    let func_name = dependency.compiled_code_function().map_or_else(
        || format_valid_function_name(dependency_contract),
        ToString::to_string,
    );

    let dep_kind = dependency.kind();
    debug!("Generating dependency file for '{dependency_contract}' with kind {dep_kind:?}");
    let content = generate_tolk_dependency_content(
        &func_name,
        boc_base64,
        dependency_contract,
        dep_kind,
        acton_config,
    );

    let output_filename = dependency
        .compiled_code_out_path()
        .unwrap_or(
            gen_dir
                .join(format!("{dependency_contract}_code.tolk"))
                .to_str()
                .ok_or_else(|| anyhow!("Path.to_str() failed"))?,
        )
        .to_string();

    let path = Path::new(&output_filename);
    let dir = path.parent();

    if let Some(dir) = dir {
        fs::create_dir_all(dir)?;
    }

    fs::write(&output_filename, content)?;

    Ok(())
}

fn format_valid_function_name(dependency_key: &str) -> String {
    let mut name = dependency_key.replace(['-', '.', ' '], "_");

    if !name.chars().next().unwrap_or(' ').is_alphabetic() {
        name = format!("contract_{name}");
    }

    format!("{name}CompiledCode")
}

fn generate_tolk_dependency_content(
    func_name: &str,
    boc_base64: &str,
    dependency_key: &str,
    kind: DependencyKind,
    acton_config: &ActonConfig,
) -> String {
    let asm_code = match kind {
        DependencyKind::EmbedCode => {
            format!("    \"{boc_base64}\" base64>B B>boc PUSHREF")
        }
        DependencyKind::LibraryRef => {
            format!(
                "    \"{boc_base64}\" base64>B B>boc hashu <b 2 8 u, swap 256 u, b>spec PUSHREF"
            )
        }
    };

    let license_header = if let Some(license) = &acton_config.package.license {
        format!("// SPDX-License-Identifier: {license}\n")
    } else {
        String::new()
    };

    let contract = acton_config
        .get_contract(dependency_key)
        .cloned()
        .unwrap_or_default();
    let contract_path = contract.src;
    let now = chrono::Local::now();
    let now_seconds = now.timestamp();
    let now_date = now.format("%Y-%m-%d %H:%M:%S");

    format!(
        "{license_header}// Auto-generated dependency code for contract '{dependency_key}'
// Provides compiled BoC data for the '{dependency_key}' contract
//
// This file is automatically generated by 'acton build'
// Do not edit manually — changes will be overwritten

/// Returns `{dependency_key}`'s code as a cell.
///
/// - Contract: `{dependency_key}`
/// - Path: `{contract_path}`
/// - Timestamp: {now_seconds}
/// - Generated on: {now_date}
@pure
fun {func_name}(): cell asm \"\"\"
{asm_code}
\"\"\"
"
    )
}

fn enable_emulator_debug_mode() -> anyhow::Result<()> {
    // hacky init VM with debug enabled due to global variables :/
    let dummy_contract: &'static str = "fun onInternalMessage(in: InMessage) {}";
    let tmp_dir = TempDir::new()?;
    let tmp_file_path = tmp_dir.path().join("enable_debug.tolk");
    let mut tmp_file = File::create(&tmp_file_path)?;
    tmp_file.write_all(dummy_contract.as_bytes())?;

    let compiler = tolkc::Compiler::new(2);
    let _ = compiler.compile(tmp_file_path.as_ref(), true);
    Ok(())
}
