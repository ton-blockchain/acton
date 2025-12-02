use crate::commands::common::error_fmt;
use crate::config::{ActonConfig, ContractConfig, ContractDependency, DependencyKind};
use crate::file_build_cache::FileBuildCache;
use anyhow::anyhow;
use log::debug;
use owo_colors::OwoColorize;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::time::Instant;
use tycho_types::boc::Boc;

mod dep_graph;

pub fn build_cmd(
    contract_id: Option<String>,
    clear_cache: bool,
    graph_output: Option<String>,
) -> anyhow::Result<()> {
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
                "No contracts section found in Acton.toml. Run 'acton init' first or add contracts manually."
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
    let failure_count = 0;
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

    for contract_key in filtered_compilation_order {
        let Some(contract_config) = contracts.get(&contract_key) else {
            continue;
        };
        let contract_path = &contract_config.src;

        generate_dependency_files(&contract_key, contract_config, &compiled_contracts, &config)?;

        let code_boc64 = process_contract(&mut file_cache, contract_config, contract_path)?;

        compiled_contracts.insert(contract_key.clone(), code_boc64.clone());

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
        Ok(())
    } else {
        Err(anyhow!(
            "Build failed with {} error{}",
            failure_count,
            if failure_count == 1 { "" } else { "s" }
        ))
    }
}

fn process_contract(
    file_cache: &mut FileBuildCache,
    contract_config: &ContractConfig,
    contract_path: &String,
) -> anyhow::Result<String> {
    let code_boc64 = if contract_path.ends_with(".boc") {
        debug!("Loading BoC file: {contract_path}");
        match fs::read(contract_path) {
            Ok(boc_data) => match Boc::decode(&boc_data) {
                Ok(boc) => Boc::encode_base64(&boc),
                Err(e) => {
                    anyhow::bail!("Failed to decode BoC file {contract_path}: {e}");
                }
            },
            Err(e) => {
                anyhow::bail!("Failed to read BoC file {contract_path}: {e}");
            }
        }
    } else {
        let cached_result = file_cache.get(contract_path, false, 2, "1.2".to_string());

        if let Some(cached_result) = cached_result {
            debug!("Cache hit, use cached result for '{contract_path}'");
            cached_result.code_boc64
        } else {
            debug!("Cache miss, recompile '{contract_path}'");
            let compile_start = Instant::now();
            println!("   {} {}", "Compiling".green().bold(), contract_config.name);

            let compilation_result = tolkc::compile(Path::new(contract_path), false);
            let compile_time = compile_start.elapsed();

            match compilation_result {
                tolkc::CompilerResult::Success(result) => {
                    if let Err(e) =
                        file_cache.put(contract_path, &result, false, 2, "1.2".to_string())
                    {
                        eprintln!(
                            "Warning: Failed to cache compilation result for {}: {}",
                            contract_config.name, e
                        );
                    }

                    println!("    {} in {:?}", "Finished".green(), compile_time);

                    result.code_boc64
                }
                tolkc::CompilerResult::Error(error) => {
                    return Err(anyhow!("Cannot compile script file {}", error.message));
                }
            }
        }
    };
    Ok(code_boc64)
}

fn save_boc_file(contract_config: &ContractConfig, code_boc64: &str) -> anyhow::Result<()> {
    if let Some(output_path) = &contract_config.output {
        let code = Boc::decode_base64(code_boc64)?;
        fs::write(output_path, Boc::encode(code))?;
    }
    Ok(())
}

pub(crate) fn generate_dependency_files(
    key: &str,
    config: &ContractConfig,
    compiled_contracts: &HashMap<String, String>, // contract_key -> boc_base64
    acton_config: &ActonConfig,
) -> anyhow::Result<()> {
    let Some(depends) = &config.depends else {
        return Ok(());
    };
    if depends.is_empty() {
        return Ok(());
    }

    for dep in depends {
        generate_single_dependency_file(key, dep, compiled_contracts, acton_config)?;
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
    contract_key: &str,
    dependency: &ContractDependency,
    compiled_contracts: &HashMap<String, String>,
    acton_config: &ActonConfig,
) -> anyhow::Result<()> {
    let gen_dir = create_gen_dir()?;
    let dependency_key = dependency.name();
    let boc_base64 = compiled_contracts.get(dependency_key).ok_or_else(|| {
        anyhow!(
            "[INTERNAL ERROR] Dependency '{dependency_key}' must be compiled before '{contract_key}'"
        )
    })?;

    let func_name = dependency
        .compiled_code_function()
        .map(|s| s.to_string())
        .unwrap_or_else(|| format_valid_function_name(dependency_key));

    let dep_kind = dependency.kind();
    debug!("Generating dependency file for '{dependency_key}' with kind {dep_kind:?}");
    let content = generate_tolk_dependency_content(
        &func_name,
        boc_base64,
        dependency_key,
        dep_kind,
        acton_config,
    );

    let output_filename = dependency
        .compiled_code_out_path()
        .unwrap_or(
            gen_dir
                .join(format!("{dependency_key}_code.tolk"))
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
    let mut name = dependency_key
        .replace("-", "_")
        .replace(".", "_")
        .replace(" ", "_");

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

    format!(
        "{license_header}// Auto-generated dependency code for contract '{dependency_key}'
// Provides compiled BoC data for the '{dependency_key}' contract
//
// This file is automatically generated by 'acton build'
// Do not edit manually — changes will be overwritten

@pure
fun {func_name}(): cell asm \"\"\"
{asm_code}
\"\"\"
"
    )
}
