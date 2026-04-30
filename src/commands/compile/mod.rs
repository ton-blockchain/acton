use crate::commands::common::error_fmt;
use crate::file_build_cache::FileBuildCache;
use crate::paths;
use acton_config::color::OwoColorize;
use acton_config::config;
use anyhow::anyhow;
use log::info;
use serde_json;
use std::fs;
use std::path::Path;
use std::time::Instant;
use tolk_compiler::SourceMap;
use tolk_compiler::abi::ContractABI;
use tycho_types::boc::Boc;

#[allow(clippy::too_many_arguments)]
pub fn compile_cmd(
    path: &String,
    json: bool,
    base64_only: bool,
    boc: Option<String>,
    fift: Option<String>,
    source_map: Option<String>,
    abi: Option<String>,
    allow_no_entrypoint: bool,
    clear_cache: bool,
) -> anyhow::Result<()> {
    if clear_cache {
        let mut file_cache = FileBuildCache::new(None)?;
        file_cache.clear()?;
        println!("  {} Cache cleared", "✓".green().bold());
    }

    let start_time = Instant::now();

    if !fs::exists(path).unwrap_or(false) {
        anyhow::bail!(error_fmt::file_not_found(path));
    }

    let metadata = fs::metadata(path)?;
    if !metadata.is_file() {
        anyhow::bail!("{} is not a file", path.yellow());
    }

    if !path.ends_with(".tolk") {
        anyhow::bail!("File must end with {}", ".tolk".yellow());
    }

    let mut file_cache = FileBuildCache::new(None)?;

    let acton_config = config::ActonConfig::load()
        .map_err(|e| {
            eprintln!("  {} Failed to load Acton.toml: {e:#}", "⚠".yellow().bold());
        })
        .ok();

    let need_debug_info = source_map.is_some();
    let need_fift = fift.is_some();
    let cache_profile = if allow_no_entrypoint {
        "1.3+allow-no-entrypoint"
    } else {
        "1.3"
    };
    if let Some(cached_entry) = file_cache.get(path, need_debug_info, need_fift, 2, cache_profile) {
        let elapsed = start_time.elapsed();
        info!(
            "Compile {path} from file cache ({}) in {elapsed:?}",
            paths::DEFAULT_BUILD_CACHE_DIR
        );

        handle_compilation_result(
            cached_entry.code_boc64,
            cached_entry.code_hash_hex,
            cached_entry.fift_code,
            cached_entry.source_map,
            cached_entry.abi,
            abi,
            source_map,
            json,
            base64_only,
            boc,
            fift,
            true,
            None,
        )?;
        return Ok(());
    }

    let compile_start = Instant::now();
    let with_debug_info = source_map.is_some();

    let mut compiler = tolk_compiler::Compiler::new(2);
    if let Some(acton_config) = &acton_config {
        let mappings = acton_config.mappings();
        compiler = compiler.with_mappings(&mappings);
    }
    compiler = compiler.with_allow_no_entrypoint(allow_no_entrypoint);

    let compilation_result = compiler.compile(Path::new(path), with_debug_info);
    let compile_time = compile_start.elapsed();

    match compilation_result {
        tolk_compiler::CompilerResult::Success(result) => {
            let total_elapsed = start_time.elapsed();
            info!(
                "Compile {path} from source (compilation: {compile_time:?}, total: {total_elapsed:?})"
            );

            if let Err(e) =
                file_cache.put(path, &result, with_debug_info, need_fift, 2, cache_profile)
                && !json
            {
                eprintln!("Warning: Failed to cache compilation result: {e}");
            }

            handle_compilation_result(
                result.code_boc64,
                result.code_hash_hex,
                need_fift.then_some(result.fift_code),
                result.source_map,
                result.abi,
                abi,
                source_map,
                json,
                base64_only,
                boc,
                fift,
                false,
                Some(total_elapsed),
            )
        }
        tolk_compiler::CompilerResult::Error(error) => {
            let total_elapsed = start_time.elapsed();
            info!(
                "Compile {} failed after {:?}: {}",
                path, total_elapsed, error.message
            );

            if json {
                let json_output = serde_json::json!({
                    "success": false,
                    "error": error.message
                });
                println!("{}", serde_json::to_string_pretty(&json_output)?);
                std::process::exit(1);
            } else {
                anyhow::bail!(error.message);
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_compilation_result(
    code_boc64: String,
    code_hash_hex: String,
    fift_code: Option<String>,
    source_map: Option<SourceMap>,
    abi: Option<ContractABI>,
    abi_path: Option<String>,
    source_map_path: Option<String>,
    json: bool,
    base64_only: bool,
    boc: Option<String>,
    fift: Option<String>,
    from_cache: bool,
    elapsed: Option<std::time::Duration>,
) -> anyhow::Result<()> {
    let code = Boc::decode_base64(code_boc64.clone())?;
    let code_hex = Boc::encode_hex(&code);

    if let Some(source_map_path) = &source_map_path {
        write_source_map(source_map.as_ref(), source_map_path)?;
    }

    if let Some(fift_path) = &fift {
        let Some(fift_code) = fift_code.as_deref() else {
            anyhow::bail!(
                "Internal error: requested Fift output is missing from compilation result"
            );
        };

        if let Some(parent_dir) = Path::new(&fift_path).parent()
            && let Err(err) = fs::create_dir_all(parent_dir)
        {
            anyhow::bail!(
                "Failed to create directory for Fift file {}: {}",
                parent_dir.display(),
                err
            );
        }

        fs::write(fift_path, fift_code)
            .map_err(|err| anyhow!("Failed to save Fift file {}: {err}", fift_path.yellow()))?;
    }

    if let Some(abi) = &abi
        && let Some(abi_path) = &abi_path
    {
        if let Some(parent_dir) = Path::new(&abi_path).parent()
            && let Err(err) = fs::create_dir_all(parent_dir)
        {
            anyhow::bail!(
                "Failed to create directory for ABI file {}: {}",
                parent_dir.display(),
                err
            );
        }

        fs::write(abi_path, serde_json::to_string_pretty(abi)?)
            .map_err(|err| anyhow!("Failed to save ABI file {}: {err}", abi_path.yellow()))?;
    }

    if let Some(boc_path) = &boc {
        if let Some(parent_dir) = Path::new(&boc_path).parent()
            && let Err(err) = fs::create_dir_all(parent_dir)
        {
            anyhow::bail!(
                "Failed to create directory for BoC file {}: {}",
                parent_dir.display(),
                err
            );
        }

        let bytes = Boc::encode(code);
        fs::write(boc_path, bytes)
            .map_err(|err| anyhow!("Failed to save BoC file {}: {err}", boc_path.yellow()))?;
        return Ok(());
    }

    if base64_only {
        println!("{code_boc64}");
    } else if json {
        let json_output = serde_json::json!({
            "success": true,
            "code_boc64": code_boc64,
            "code_hex": code_hex,
            "code_hash_hex": code_hash_hex
        });
        println!("{}", serde_json::to_string_pretty(&json_output)?);
    } else {
        if from_cache {
            println!("{}", "✓ Compilation successful (from cache)".green().bold());
        } else {
            let elapsed_msg = elapsed
                .map(|e| format!(" (compiled in {e:?})").dimmed().to_string())
                .unwrap_or_default();
            println!(
                "{}{}",
                "✓ Compilation successful".green().bold(),
                elapsed_msg
            );
        }
        println!("Code in base64: {}", code_boc64.dimmed());
        println!("Code in hex: {}", code_hex.dimmed());
        println!("Code hash hex: {}", format!("0x{code_hash_hex}").dimmed());
    }
    Ok(())
}

fn write_source_map(source_map: Option<&SourceMap>, source_map_path: &str) -> anyhow::Result<()> {
    if let Some(parent_dir) = Path::new(source_map_path).parent()
        && let Err(err) = fs::create_dir_all(parent_dir)
    {
        anyhow::bail!(
            "Failed to create directory for source map file {}: {}",
            parent_dir.display(),
            err
        );
    }

    let source_map = source_map.ok_or_else(|| {
        anyhow!(
            "No source map data available for {}",
            source_map_path.yellow()
        )
    })?;

    let json_string = serde_json::to_string_pretty(&source_map).map_err(|err| {
        anyhow!(
            "Failed to serialize source map {}: {err}",
            source_map_path.yellow()
        )
    })?;
    fs::write(source_map_path, json_string).map_err(|err| {
        anyhow!(
            "Failed to save source map {}: {err}",
            source_map_path.yellow()
        )
    })?;
    Ok(())
}
