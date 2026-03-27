use crate::commands::common::error_fmt;
use acton_config::color::OwoColorize;
use anyhow::anyhow;
use std::fs;
use std::path::Path;
use std::str::FromStr;
use tasm::decompile::Disassembler;
use tasm::printer::FormatOptions;
use tasm::types::Instruction;
use ton_api::{Network, TonApiClient};
use tycho_types::boc::Boc;
use tycho_types::cell::HashBytes;

mod remote;

#[allow(clippy::too_many_arguments)]
pub fn disasm_cmd(
    boc_file: Option<String>,
    boc_string: Option<String>,
    output_file: Option<String>,
    opts: FormatOptions,
    address: Option<String>,
    api_key: Option<String>,
    net: Option<String>,
    follow_libraries: bool,
) -> anyhow::Result<()> {
    if boc_file.is_some() && boc_string.is_some() {
        anyhow::bail!(
            "Cannot provide both {}/{} and {} argument",
            "--string".yellow(),
            "-s".yellow(),
            "BOC_FILE".yellow()
        );
    }

    let network = net.as_deref().map(Network::from_str).transpose()?;

    let mut resolved_network = network.clone();

    let boc_data = if let Some(string) = boc_string {
        string
    } else if let Some(path) = boc_file {
        if !fs::exists(&path).unwrap_or(false) {
            anyhow::bail!(error_fmt::file_not_found(&path));
        }

        let metadata =
            fs::metadata(&path).map_err(|err| anyhow!("Cannot access {}: {err}", path.yellow()))?;
        if !metadata.is_file() {
            anyhow::bail!("{} is not a file", path.yellow());
        }

        // BoC file can be binary file or file with hex/base64 encoded data
        let binary_data =
            fs::read(&path).map_err(|err| anyhow!("Cannot access {}: {err}", path.yellow()))?;
        if let Ok(cell) = Boc::decode_base64(binary_data.trim_ascii()) {
            Boc::encode_hex(cell)
        } else if let Ok(cell) = Boc::decode_hex(binary_data.trim_ascii()) {
            Boc::encode_hex(cell)
        } else {
            hex::encode(binary_data)
        }
    } else if let Some(addr) = address {
        let fetched = remote::fetch_contract_boc(network.clone(), &addr, api_key.as_deref())?;
        resolved_network = Some(fetched.network);
        fetched.boc
    } else {
        anyhow::bail!(
            "Either {}, {}, {} or {} argument must be provided, run with {} for more information",
            "--string".yellow(),
            "-s".yellow(),
            "--address".yellow(),
            "BOC_FILE".yellow(),
            "--help".yellow()
        );
    };

    let cell = if let Ok(cell) = Boc::decode_hex(&boc_data) {
        cell
    } else if let Ok(cell) = Boc::decode_base64(&boc_data) {
        cell
    } else {
        return Err(anyhow::anyhow!(
            "Failed to decode BoC data as hex or base64"
        ));
    };

    let disassembler = Disassembler::new();
    let mut final_cell = cell;

    // In --follow-libraries mode for code like
    // exotic library x{...}
    // we look up for library and disassemble actual code instead of just cell reference
    if follow_libraries {
        let code = disassembler.decompile_cell(&final_cell)?;
        let instructions = code.instructions;

        if instructions.len() == 1
            && let Some(lib_hash) = extract_library_hash_from_instruction(&instructions[0])
        {
            let config = acton_config::config::ActonConfig::load().unwrap_or_default();
            let custom_networks = config.custom_networks();
            let client = TonApiClient::new(
                resolved_network.unwrap_or(Network::Testnet),
                custom_networks,
                api_key,
            )?;
            match client.get_library_by_hash(&lib_hash) {
                Ok(lib_cell) => {
                    final_cell = lib_cell;
                }
                Err(err) => {
                    eprintln!("Warning: Failed to load library 0x{lib_hash}: {err}");
                    eprintln!("Showing original code instead");
                }
            }
        }
    }

    let code = disassembler.decompile_cell(&final_cell)?;
    let output = code.print(&opts);

    if let Some(output_path) = output_file {
        // Create parent directories if they don't exist
        if let Some(parent_dir) = Path::new(&output_path).parent()
            && let Err(err) = fs::create_dir_all(parent_dir)
        {
            anyhow::bail!(
                "Failed to create output directory {}: {}",
                parent_dir.display(),
                err
            );
        }

        fs::write(&output_path, &output)?;
        println!("Disassembled code written to {output_path}");
    } else {
        println!("{output}");
    }

    Ok(())
}

fn extract_library_hash_from_instruction(instruction: &Instruction) -> Option<HashBytes> {
    match instruction {
        Instruction::ExoticCell(instr) => {
            let mut slice = instr.cell.as_slice_allow_exotic();
            let typ = slice.load_u8().ok()?;
            if typ == 2 {
                let hash = slice.load_u256().ok()?;
                return Some(hash);
            }

            None
        }
        Instruction::Plain(_) => None,
        Instruction::Ref(_) => None,
    }
}
