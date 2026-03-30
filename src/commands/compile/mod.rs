use crate::commands::common::error_fmt;
use crate::file_build_cache::FileBuildCache;
use acton_config::color::OwoColorize;
use acton_config::config;
use anyhow::anyhow;
use log::info;
use serde_json;
use std::fs;
use std::path::Path;
use std::time::Instant;
use tolkc::abi::ContractABI;
use tolkc::{SourceMap as TolkCompilerSourceMap, TolkSourceMap};
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

    let acton_config = config::ActonConfig::load().ok();

    let need_debug_info = source_map.is_some();
    if let Some(cached_entry) = file_cache.get(path, need_debug_info, 2, "1.3") {
        let elapsed = start_time.elapsed();
        info!("Compile {path} from file cache (.acton/cache) in {elapsed:?}");

        handle_compilation_result(
            cached_entry.code_boc64,
            cached_entry.code_hash_hex,
            cached_entry.fift_code,
            cached_entry.debug_mark_base64,
            cached_entry.new_source_map,
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

    let mut compiler = tolkc::Compiler::new(2);
    if let Some(acton_config) = &acton_config {
        let mappings = acton_config.mappings();
        compiler = compiler.with_mappings(&mappings);
    }

    let compilation_result = compiler.compile(Path::new(path), with_debug_info);
    let compile_time = compile_start.elapsed();

    match compilation_result {
        tolkc::CompilerResult::Success(result) => {
            let total_elapsed = start_time.elapsed();
            info!(
                "Compile {path} from source (compilation: {compile_time:?}, total: {total_elapsed:?})"
            );

            if let Err(e) = file_cache.put(path, &result, with_debug_info, 2, "1.3")
                && !json
            {
                eprintln!("Warning: Failed to cache compilation result: {e}");
            }

            handle_compilation_result(
                result.code_boc64,
                result.code_hash_hex,
                result.fift_code,
                result.debug_mark_base64,
                result.new_source_map,
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
        tolkc::CompilerResult::Error(error) => {
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
    fift_code: String,
    debug_mark_base64: Option<String>,
    new_source_map: Option<TolkCompilerSourceMap>,
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
        write_source_map(
            new_source_map.as_ref(),
            &code,
            debug_mark_base64.as_deref(),
            source_map_path,
        )?;
    }

    if let Some(fift_path) = &fift {
        if let Some(parent_dir) = Path::new(&fift_path).parent()
            && let Err(err) = fs::create_dir_all(parent_dir)
        {
            anyhow::bail!(
                "Failed to create directory for Fift file {}: {}",
                parent_dir.display(),
                err
            );
        }

        fs::write(fift_path, &fift_code)
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

fn write_source_map(
    new_source_map: Option<&TolkCompilerSourceMap>,
    code: &tycho_types::cell::Cell,
    debug_mark_base64: Option<&str>,
    source_map_path: &str,
) -> anyhow::Result<()> {
    if let Some(parent_dir) = Path::new(source_map_path).parent()
        && let Err(err) = fs::create_dir_all(parent_dir)
    {
        anyhow::bail!(
            "Failed to create directory for source map file {}: {}",
            parent_dir.display(),
            err
        );
    }

    let source_map = new_source_map.ok_or_else(|| {
        anyhow!(
            "No source map data available for {}",
            source_map_path.yellow()
        )
    })?;

    let tolk_source_map =
        TolkSourceMap::from_code_cell(source_map.clone(), code, debug_mark_base64)?;
    let json_string = serde_json::to_string_pretty(&tolk_source_map).map_err(|err| {
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

#[cfg(test)]
mod tests {
    use super::write_source_map;
    use tycho_types::cell::Cell;

    #[test]
    fn write_source_map_requires_source_map_data() {
        let error =
            write_source_map(None, &Cell::empty(), None, "source-map.json").expect_err("must fail");
        let rendered = error.to_string();

        assert!(
            rendered.contains("No source map data available for"),
            "unexpected error: {rendered}"
        );
        assert!(
            rendered.contains("source-map.json"),
            "unexpected error: {rendered}"
        );
    }
}
