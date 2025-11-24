use crate::file_build_cache::FileBuildCache;
use anyhow::anyhow;
use log::info;
use owo_colors::OwoColorize;
use serde_json;
use std::fs;
use std::path::Path;
use std::time::Instant;
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

    let metadata = fs::metadata(path)?;
    if !metadata.is_file() {
        return Err(anyhow!("Path '{path}' is not a file"));
    }

    if !path.ends_with(".tolk") {
        return Err(anyhow!("File must end with .tolk"));
    }

    let mut file_cache = FileBuildCache::new(None)?;

    if let Some(cached_entry) = file_cache.get(path, false, 2, "1.2".to_string()) {
        let elapsed = start_time.elapsed();
        info!("Compile {path} from file cache (.acton/cache) in {elapsed:?}");

        handle_compilation_result(
            cached_entry.code_boc64,
            cached_entry.code_hash_hex,
            cached_entry.fift_code,
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

            if let Some(source_map_path) = &source_map {
                if let Some(source_map_data) = &result.source_map {
                    if let Ok(json_string) = serde_json::to_string_pretty(source_map_data) {
                        if let Err(e) = fs::write(source_map_path, json_string) {
                            eprintln!(
                                "Warning: Failed to write source map to {source_map_path}: {e}"
                            );
                        }
                    } else {
                        eprintln!("Warning: Failed to serialize source map");
                    }
                } else if !json && !base64_only {
                    eprintln!("Warning: No source map data available");
                }
            }

            handle_compilation_result(
                result.code_boc64,
                result.code_hash_hex,
                result.fift_code,
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
            } else {
                println!(
                    "{} {}",
                    "✗ Compilation failed".red().bold(),
                    error.message.red()
                );
            }
            std::process::exit(1);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_compilation_result(
    code_boc64: String,
    code_hash_hex: String,
    fift_code: String,
    json: bool,
    base64_only: bool,
    boc: Option<String>,
    fift: Option<String>,
    from_cache: bool,
    elapsed: Option<std::time::Duration>,
) -> anyhow::Result<()> {
    let code = Boc::decode_base64(code_boc64.clone())?;
    let code_hex = Boc::encode_hex(&code);

    if let Some(fift_path) = fift {
        fs::write(fift_path, &fift_code)?;
    }

    if let Some(boc_path) = boc {
        let bytes = Boc::encode(code);
        fs::write(boc_path, bytes)?;
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
                .map(|e| format!(" ({})", format!("compiled in {e:?}").dimmed()))
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
