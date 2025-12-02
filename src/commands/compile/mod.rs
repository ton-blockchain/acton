use crate::commands::common::error_fmt;
use crate::file_build_cache::FileBuildCache;
use anyhow::anyhow;
use log::info;
use owo_colors::OwoColorize;
use serde_json;
use std::fs;
use std::path::Path;
use std::time::Instant;
use tolkc::source_map::SourceMap;
use tycho_types::boc::Boc;

pub fn compile_cmd(
    path: &String,
    json: bool,
    base64_only: bool,
    boc: Option<String>,
    fift: Option<String>,
    source_map: Option<String>,
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

    if let Some(cached_entry) = file_cache.get(path, false, 2, "1.2".to_string()) {
        let elapsed = start_time.elapsed();
        info!("Compile {path} from file cache (.acton/cache) in {elapsed:?}");

        handle_compilation_result(
            cached_entry.code_boc64,
            cached_entry.code_hash_hex,
            cached_entry.fift_code,
            cached_entry.source_map,
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
    let compilation_result = tolkc::compile(Path::new(path), with_debug_info);
    let compile_time = compile_start.elapsed();

    match compilation_result {
        tolkc::CompilerResult::Success(result) => {
            let total_elapsed = start_time.elapsed();
            info!(
                "Compile {path} from source (compilation: {compile_time:?}, total: {total_elapsed:?})"
            );

            if let Err(e) = file_cache.put(path, &result, with_debug_info, 2, "1.2".to_string())
                && !json
            {
                eprintln!("Warning: Failed to cache compilation result: {e}");
            }

            handle_compilation_result(
                result.code_boc64,
                result.code_hash_hex,
                result.fift_code,
                result.source_map,
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
    source_map: Option<SourceMap>,
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
        if let Some(source_map_data) = &source_map {
            if let Ok(json_string) = serde_json::to_string_pretty(source_map_data) {
                fs::write(source_map_path, json_string).map_err(|err| {
                    anyhow!(color_print::cformat!(
                        "Failed to save source map <yellow>{source_map_path}</>: {err}"
                    ))
                })?;
            } else {
                eprintln!("Warning: Failed to serialize source map");
            }
        } else if !json && !base64_only {
            eprintln!("Warning: No source map data available");
        }
    }

    if let Some(fift_path) = &fift {
        fs::write(fift_path, &fift_code).map_err(|err| {
            anyhow!(color_print::cformat!(
                "Failed to save Fift file <yellow>{fift_path}</>: {err}"
            ))
        })?;
    }

    if let Some(boc_path) = &boc {
        let bytes = Boc::encode(code);
        fs::write(boc_path, bytes).map_err(|err| {
            anyhow!(color_print::cformat!(
                "Failed to save BoC file <yellow>{boc_path}</>: {err}"
            ))
        })?;
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
        println!("Code hash hex: {}", code_hash_hex.dimmed());
    }
    Ok(())
}
