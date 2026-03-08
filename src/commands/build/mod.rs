use crate::commands::common::error_fmt;
use crate::file_build_cache::FileBuildCache;
use crate::stdlib;
use acton_config::color::OwoColorize;
use acton_config::config::{
    ActonConfig, ContractConfig, ContractDependency, DependencyKind,
    project_root as configured_project_root,
};
use anyhow::anyhow;
use heck::ToLowerCamelCase;
use log::debug;
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Instant;
use tempfile::TempDir;
use tycho_types::boc::Boc;

mod dep_graph;

pub fn build_cmd(
    contract_id: Option<String>,
    clear_cache: bool,
    graph_output: Option<String>,
    out_dir: Option<String>,
    gen_dir: Option<String>,
    output_fift: Option<String>,
    show_info: bool,
) -> anyhow::Result<()> {
    let project_root = configured_project_root();
    stdlib::ensure_latest(project_root)?;

    // Due to global variables, we need to enable debug mode for emulator as early as possible
    // since first compilation WITHOUT debug mode will set debug=false forever
    enable_emulator_debug_mode()?;

    if clear_cache {
        let mut file_cache = FileBuildCache::new(None)?;
        file_cache.clear()?;
        println!("  {} Cache cleared", "✓".green().bold());
    }

    println!("   {} contracts", "Compiling".green().bold());

    let config = ActonConfig::load()?;
    let out_dir = resolve_build_output_dir(
        out_dir,
        config
            .build
            .as_ref()
            .and_then(|build| non_empty_path(build.out_dir.clone())),
        "build",
        project_root,
    );
    let gen_dir = resolve_build_output_dir(
        gen_dir,
        config
            .build
            .as_ref()
            .and_then(|build| non_empty_path(build.gen_dir.clone())),
        "gen",
        project_root,
    );
    let output_fift_dir = resolve_optional_build_output_dir(
        output_fift,
        config
            .build
            .as_ref()
            .and_then(|build| non_empty_path(build.output_fift.clone())),
        project_root,
    );

    if !out_dir.exists() {
        fs::create_dir_all(&out_dir)?;
    }

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
            "deps.dot"
        } else {
            graph_path
        };
        dep_graph::generate_dependency_graph_dot(
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
        let contract_path = resolve_project_config_path(project_root, &contract_config.src);

        generate_dependency_files(
            &parent_contract,
            contract_config,
            &compiled_contracts,
            &compile_errors,
            &config,
            &gen_dir,
            project_root,
        )?;

        let (code_boc64, code_hash, fift_code) = match process_contract(
            &mut file_cache,
            contract_config,
            &contract_config.src,
            &contract_path,
            &config,
        ) {
            Ok((code, hash, fift_code)) => (code, hash, fift_code),
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

        if let Err(e) = save_boc_file(project_root, contract_config, &code_boc64) {
            eprintln!(
                "Warning: Failed to save cached BoC file for {}: {}",
                contract_config.name, e
            );
        }

        if let Some(output_fift_dir) = &output_fift_dir
            && let Some(fift_code) = &fift_code
            && let Err(e) = save_fift_file(output_fift_dir, &parent_contract, fift_code)
        {
            eprintln!(
                "Warning: Failed to save Fift file for {}: {}",
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
                println!("        {} {}", "Hash".cyan(), format!("0x{hash}").dimmed());
            }
        }

        Ok(())
    } else {
        let mut whole_error = String::new();

        for (contract, err) in compile_errors {
            whole_error += format!("In {}:\n\n{err}\n", contract.yellow()).as_str();
        }

        whole_error.push_str(
            format!(
                "{} with {} error{}",
                "Build failed".red(),
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
    contract_src: &str,
    contract_path: &Path,
    acton_config: &ActonConfig,
) -> anyhow::Result<(String, String, Option<String>)> {
    let (code_boc64, code_hash, fift_code) = if contract_src.ends_with(".boc") {
        debug!("Loading BoC file: {}", contract_path.display());
        match fs::read(contract_path) {
            Ok(boc_data) => match Boc::decode(&boc_data) {
                Ok(boc) => {
                    let code_boc64 = Boc::encode_base64(&boc);
                    (code_boc64, boc.repr_hash().to_string(), None)
                }
                Err(e) => {
                    anyhow::bail!("Failed to decode BoC file {contract_src}: {e}");
                }
            },
            Err(e) => {
                anyhow::bail!("Failed to read BoC file {contract_src}: {e}");
            }
        }
    } else {
        let cached_result = file_cache.get(contract_src, false, 2, "1.3");

        if let Some(cached_result) = cached_result {
            debug!("Cache hit, use cached result for '{contract_src}'");
            (
                cached_result.code_boc64,
                cached_result.code_hash_hex,
                Some(cached_result.fift_code),
            )
        } else {
            debug!("Cache miss, recompile '{}'", contract_path.display());
            let compile_start = Instant::now();
            println!("   {} {}", "Compiling".green().bold(), contract_config.name);

            let mappings = acton_config.mappings();
            let compiler = tolkc::Compiler::new(2).with_mappings(&mappings);
            let compilation_result = compiler.compile(contract_path, false);
            let compile_time = compile_start.elapsed();

            match compilation_result {
                tolkc::CompilerResult::Success(result) => {
                    if let Err(e) = file_cache.put(contract_src, &result, false, 2, "1.3") {
                        eprintln!(
                            "Warning: Failed to cache compilation result for {}: {}",
                            contract_config.name, e
                        );
                    }

                    println!("    {} in {:?}", "Finished".green(), compile_time);

                    (
                        result.code_boc64,
                        result.code_hash_hex,
                        Some(result.fift_code),
                    )
                }
                tolkc::CompilerResult::Error(error) => {
                    let message = rewrite_compiler_error_paths_for_display(
                        &error.message,
                        contract_src,
                        contract_path,
                    );
                    anyhow::bail!(message);
                }
            }
        }
    };
    Ok((code_boc64, code_hash, fift_code))
}

fn save_boc_file(
    project_root: &Path,
    contract_config: &ContractConfig,
    code_boc64: &str,
) -> anyhow::Result<()> {
    if let Some(config_output_path) = &contract_config.output {
        let output_path = resolve_project_config_path(project_root, config_output_path);
        let display_parent_dir = Path::new(config_output_path)
            .parent()
            .or_else(|| output_path.parent());
        if let Some(parent_dir) = output_path.parent()
            && let Err(err) = fs::create_dir_all(parent_dir)
        {
            anyhow::bail!(
                "Failed to create directory for BoC file {}: {}",
                display_parent_dir.map_or_else(
                    || parent_dir.display().to_string(),
                    |path| path.display().to_string()
                ),
                err
            );
        }

        let code = Boc::decode_base64(code_boc64)?;
        fs::write(output_path, Boc::encode(code))?;
    }
    Ok(())
}

fn save_build_artifact(
    out_dir: &Path,
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
    let path = out_dir.join(filename);
    fs::write(path, serde_json::to_string_pretty(&json_data)?)?;

    Ok(())
}

fn save_fift_file(
    output_fift_dir: &Path,
    contract_key: &str,
    fift_code: &str,
) -> anyhow::Result<()> {
    let filename = format!("{contract_key}.fif");
    let path = output_fift_dir.join(filename);

    if let Some(parent_dir) = path.parent()
        && let Err(err) = fs::create_dir_all(parent_dir)
    {
        anyhow::bail!(
            "Failed to create directory for Fift file {}: {}",
            parent_dir.display(),
            err
        );
    }

    fs::write(&path, fift_code)
        .map_err(|err| anyhow!("Failed to save Fift file {}: {}", path.display(), err))?;

    Ok(())
}

pub(crate) fn generate_dependency_files(
    parent_contract: &str,
    config: &ContractConfig,
    compiled_contracts: &HashMap<String, String>, // contract_key -> boc_base64
    failed_contracts: &BTreeMap<String, anyhow::Error>,
    acton_config: &ActonConfig,
    gen_dir: &Path,
    project_root: &Path,
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
            gen_dir,
            project_root,
        )?;
    }

    Ok(())
}

fn create_gen_dir(gen_dir: &Path) -> anyhow::Result<()> {
    if !gen_dir.exists() {
        fs::create_dir_all(gen_dir)?;
    }
    Ok(())
}

fn generate_single_dependency_file(
    parent_contract: &str,
    dependency: &ContractDependency,
    compiled_contracts: &HashMap<String, String>,
    failed_contracts: &BTreeMap<String, anyhow::Error>,
    acton_config: &ActonConfig,
    gen_dir: &Path,
    project_root: &Path,
) -> anyhow::Result<()> {
    create_gen_dir(gen_dir)?;
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

    let output_path = if let Some(output_path) = dependency.compiled_code_out_path() {
        resolve_project_config_path(project_root, output_path)
    } else {
        gen_dir.join(format!("{dependency_contract}_code.tolk"))
    };
    let dir = output_path.parent();

    if let Some(dir) = dir {
        fs::create_dir_all(dir)?;
    }

    fs::write(output_path, content)?;

    Ok(())
}

fn non_empty_path(path: Option<String>) -> Option<String> {
    path.filter(|value| !value.is_empty())
}

fn rewrite_compiler_error_paths_for_display(
    message: &str,
    contract_src: &str,
    contract_path: &Path,
) -> String {
    if Path::new(contract_src).is_absolute() || !message.contains("Failed to locate ") {
        return message.to_string();
    }

    let absolute_contract_path = contract_path.to_string_lossy();
    let absolute_prefix = format!("Failed to locate {}", absolute_contract_path);
    let relative_prefix = format!("Failed to locate {contract_src}");

    if message.contains(&absolute_prefix) {
        message.replacen(&absolute_prefix, &relative_prefix, 1)
    } else {
        message.to_string()
    }
}

fn resolve_build_output_dir(
    cli_path: Option<String>,
    config_path: Option<String>,
    default_dir: &str,
    project_root: &Path,
) -> PathBuf {
    if let Some(cli_path) = non_empty_path(cli_path) {
        return PathBuf::from(cli_path);
    }
    if let Some(config_path) = non_empty_path(config_path) {
        return resolve_project_config_path(project_root, &config_path);
    }
    project_root.join(default_dir)
}

fn resolve_optional_build_output_dir(
    cli_path: Option<String>,
    config_path: Option<String>,
    project_root: &Path,
) -> Option<PathBuf> {
    if let Some(cli_path) = non_empty_path(cli_path) {
        return Some(PathBuf::from(cli_path));
    }
    non_empty_path(config_path)
        .map(|config_path| resolve_project_config_path(project_root, &config_path))
}

fn resolve_project_config_path(project_root: &Path, path: &str) -> PathBuf {
    let path = Path::new(path);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        project_root.join(path)
    }
}

fn format_valid_function_name(dependency_key: &str) -> String {
    let mut name = dependency_key.replace(['-', '.', ' '], "_");

    if !name.chars().next().unwrap_or(' ').is_alphabetic() {
        name = format!("contract_{name}");
    }

    name = name.to_lower_camel_case();

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
///
/// # Safety
///
/// This function always returns the correct cell with the contract code.
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
