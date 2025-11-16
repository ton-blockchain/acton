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
    contract_filter: Option<String>,
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
                "No contracts found in Acton.toml. Run 'acton init' first or add contracts manually."
            );
            return Ok(());
        }
    };

    if contracts.is_empty() {
        println!("No contracts to build.");
        return Ok(());
    }

    let mut file_cache = FileBuildCache::new(None)?;
    let mut failure_count = 0;
    let total_start = Instant::now();

    if let Some(filter) = &contract_filter {
        if contracts.iter().find(|(key, _)| key == &filter).is_none() {
            return Err(anyhow!("Contract '{}' not found in Acton.toml", filter));
        }
    }

    let flatten_contracts = contracts.iter().collect::<Vec<_>>();
    let compilation_order = dep_graph::build_dependency_graph(&flatten_contracts)?;
    debug!("Compilation order: {:?}", compilation_order);

    let filtered_compilation_order = if let Some(filter) = &contract_filter {
        dep_graph::filter_compilation_order_for_contract(filter, &compilation_order, contracts)?
    } else {
        compilation_order
    };

    debug!("Build next contracts: {:?}", filtered_compilation_order);

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
        let contract_config = contracts.get(&contract_key).unwrap();
        let contract_path = &contract_config.src;

        generate_dependency_files(
            &contract_key,
            &contract_config,
            &compiled_contracts,
            &config,
        )?;

        let code_boc64 = process_contract(
            &mut file_cache,
            &mut failure_count,
            contract_config,
            contract_path,
        );

        let Some(code_boc64) = &code_boc64 else {
            continue;
        };

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
    failure_count: &mut i32,
    contract_config: &ContractConfig,
    contract_path: &String,
) -> Option<String> {
    let code_boc64 = if contract_path.ends_with(".boc") {
        debug!("Loading BoC file: {}", contract_path);
        match fs::read(contract_path) {
            Ok(boc_data) => match Boc::decode(&boc_data) {
                Ok(boc) => {
                    let boc_base64 = Boc::encode_base64(&boc);
                    Some(boc_base64)
                }
                Err(e) => {
                    eprintln!(
                        "Warning: Failed to decode BoC file {}: {}",
                        contract_path, e
                    );
                    None
                }
            },
            Err(e) => {
                eprintln!("Warning: Failed to read BoC file {}: {}", contract_path, e);
                None
            }
        }
    } else {
        let cached_result = file_cache.get(contract_path, false, 2, "1.2".to_string());

        if let Some(cached_result) = cached_result {
            debug!("Cache hit, use cached result for '{}'", contract_path);
            Some(cached_result.code_boc64)
        } else {
            debug!("Cache miss, recompile '{}'", contract_path);
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

                    Some(result.code_boc64)
                }
                tolkc::CompilerResult::Error(error) => {
                    eprintln!("{}", error.message);
                    *failure_count += 1;
                    None
                }
            }
        }
    };
    code_boc64
}

fn save_boc_file(contract_config: &ContractConfig, code_boc64: &str) -> anyhow::Result<()> {
    if let Some(output_path) = &contract_config.output {
        let code = Boc::decode_base64(code_boc64)?;
        fs::write(output_path, Boc::encode(code))?;
    }
    Ok(())
}

pub(crate) fn generate_dependency_files(
    key: &String,
    config: &ContractConfig,
    compiled_contracts: &HashMap<String, String>, // contract_key -> boc_base64
    acton_config: &ActonConfig,
) -> anyhow::Result<()> {
    let gen_dir = Path::new("gen");
    if !gen_dir.exists() {
        fs::create_dir_all(gen_dir)?;
    }

    let Some(depends) = &config.depends else {
        return Ok(());
    };
    if depends.is_empty() {
        return Ok(());
    }

    for dep in depends {
        generate_single_dependency_file(key, dep, compiled_contracts, acton_config, gen_dir)?;
    }

    Ok(())
}

fn generate_single_dependency_file(
    contract_key: &str,
    dependency: &ContractDependency,
    compiled_contracts: &HashMap<String, String>,
    acton_config: &ActonConfig,
    gen_dir: &Path,
) -> anyhow::Result<()> {
    let dependency_key = dependency.name();
    let boc_base64 = compiled_contracts.get(dependency_key).ok_or_else(|| {
        anyhow!(
            "[INTERNAL ERROR] Dependency '{}' must be compiled before '{}'",
            dependency_key,
            contract_key
        )
    })?;

    let func_name = dependency
        .compiled_code_function()
        .map(|s| s.to_string())
        .unwrap_or_else(|| format_valid_function_name(dependency_key));

    let dep_kind = dependency.kind();
    debug!(
        "Generating dependency file for '{}' with kind {:?}",
        dependency_key, dep_kind
    );
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
                .join(format!("{}_code.tolk", dependency_key))
                .to_str()
                .unwrap(),
        )
        .to_string();

    fs::write(&output_filename, content)?;

    Ok(())
}

fn format_valid_function_name(dependency_key: &str) -> String {
    let mut name = dependency_key
        .replace("-", "_")
        .replace(".", "_")
        .replace(" ", "_");

    if !name.chars().next().unwrap_or(' ').is_alphabetic() {
        name = format!("contract_{}", name);
    }

    format!("{}CompiledCode", name)
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
            format!("    \"{}\" base64>B B>boc PUSHREF", boc_base64)
        }
        DependencyKind::LibraryRef => {
            format!(
                "    \"{}\" base64>B B>boc hashu <b 2 8 u, swap 256 u, b>spec PUSHREF",
                boc_base64
            )
        }
    };

    let license_header = if let Some(license) = &acton_config.package.license {
        format!("// SPDX-License-Identifier: {}\n", license)
    } else {
        String::new()
    };

    format!(
        "{}// Auto-generated dependency code for contract '{}'
// Provides compiled BoC data for the '{}' contract
//
// This file is automatically generated by 'acton build'
// Do not edit manually — changes will be overwritten

@pure
fun {}(): cell asm \"\"\"
{}
\"\"\"
",
        license_header, dependency_key, dependency_key, func_name, asm_code
    )
}
