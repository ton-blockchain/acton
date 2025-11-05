use anyhow::anyhow;
use owo_colors::OwoColorize;
use serde_json;
use std::fs;
use std::path::Path;
use tycho_types::boc::Boc;

pub fn compile_cmd(
    path: &String,
    json: bool,
    base64_only: bool,
    boc: Option<String>,
) -> anyhow::Result<()> {
    let metadata = fs::metadata(path)?;
    if !metadata.is_file() {
        return Err(anyhow!("Path '{}' is not a file", path));
    }

    if !path.ends_with(".tolk") {
        return Err(anyhow!("File must end with .tolk"));
    }

    if !json {
        println!("  {} {}", "Compiling".bold().cyan(), path.dimmed());
    }

    let compilation_result = tolkc::compile(Path::new(path), false);

    match compilation_result {
        tolkc::CompilerResult::Success(result) => {
            let code = Boc::decode_base64(result.code_boc64.clone())?;
            let code_hex = Boc::encode_hex(&code);

            if let Some(boc) = boc {
                let bytes = Boc::encode(code);
                fs::write(boc, bytes)?;
                return Ok(());
            }

            if base64_only {
                println!("{}", result.code_boc64);
            } else if json {
                let json_output = serde_json::json!({
                    "success": true,
                    "code_boc64": result.code_boc64,
                    "code_hex": code_hex,
                    "code_hash_hex": result.code_hash_hex
                });
                println!("{}", serde_json::to_string_pretty(&json_output).unwrap());
            } else {
                println!("{}", "✓ Compilation successful".green().bold());
                println!("Code in base64: {}", result.code_boc64.dimmed());
                println!("Code in hex: {}", code_hex.dimmed());
                println!("Code hash hex: {}", result.code_hash_hex.dimmed());
            }
            Ok(())
        }
        tolkc::CompilerResult::Error(error) => {
            if json {
                let json_output = serde_json::json!({
                    "success": false,
                    "error": error.message
                });
                println!("{}", serde_json::to_string_pretty(&json_output).unwrap());
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
