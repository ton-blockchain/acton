use crate::commands::common::error_fmt;
use acton_config::color::OwoColorize;
use anyhow::anyhow;
use serde::Serialize;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::str::FromStr;
use tasm_core::decompile::Disassembler;
use tasm_core::printer::FormatOptions;
use tasm_core::types::{ArgValue, Code, Instruction};
use tolk_compiler::SourceMap;
use tolk_source_map::SourceLocation;
use ton_api::{Network, TonApiClient};
use tycho_types::boc::Boc;
use tycho_types::cell::{Cell, HashBytes};

mod remote;

#[allow(clippy::too_many_arguments)]
pub fn disasm_cmd(
    boc_file: Option<String>,
    boc_string: Option<String>,
    output_file: Option<String>,
    opts: FormatOptions,
    address: Option<String>,
    net: Option<String>,
    follow_libraries: bool,
    json: bool,
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
        if string.trim().is_empty() {
            anyhow::bail!("{} cannot be empty", "--string".yellow());
        }

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
        if addr.trim().is_empty() {
            anyhow::bail!("{} cannot be empty", "--address".yellow());
        }

        let fetched = remote::fetch_contract_boc(network, &addr)?;
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
    if json {
        let result = build_json_output(&code, &opts);
        if let Some(output_path) = output_file {
            write_output_file(&output_path, &result.assembly)?;
        }
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        let output = code.print(&opts);
        if let Some(output_path) = output_file {
            write_output_file(&output_path, &output)?;
            println!("Disassembled code written to {output_path}");
        } else {
            println!("{output}");
        }
    }

    Ok(())
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct SourceLocationKey {
    file: String,
    line: i64,
    column: i64,
    end_line: i64,
    end_column: i64,
}

impl From<&SourceLocation> for SourceLocationKey {
    fn from(value: &SourceLocation) -> Self {
        Self {
            file: value.file.clone(),
            line: value.line,
            column: value.column,
            end_line: value.end_line,
            end_column: value.end_column,
        }
    }
}

#[derive(Serialize)]
struct DisasmJsonOutput {
    success: bool,
    assembly: String,
    blocks: Vec<DisasmSourceBlock>,
}

#[derive(Serialize)]
struct DisasmSourceBlock {
    source: DisasmSourceLocation,
    assembly_ranges: Vec<DisasmAssemblyRange>,
}

#[derive(Serialize)]
struct DisasmSourceLocation {
    file: String,
    line: i64,
    column: i64,
    end_line: i64,
    end_column: i64,
}

impl From<&SourceLocation> for DisasmSourceLocation {
    fn from(value: &SourceLocation) -> Self {
        Self {
            file: value.file.clone(),
            line: value.line,
            column: value.column,
            end_line: value.end_line,
            end_column: value.end_column,
        }
    }
}

#[derive(Serialize)]
struct DisasmAssemblyRange {
    start_line: usize,
    end_line: usize,
}

struct DisasmSourceBlockBuilder {
    source: DisasmSourceLocation,
    assembly_ranges: Vec<DisasmAssemblyRange>,
}

impl DisasmSourceBlockBuilder {
    fn new(location: &SourceLocation) -> Self {
        Self {
            source: DisasmSourceLocation::from(location),
            assembly_ranges: Vec::new(),
        }
    }

    fn add_line(&mut self, line: usize) {
        if let Some(last_range) = self.assembly_ranges.last_mut()
            && last_range.end_line + 1 == line
        {
            last_range.end_line = line;
            return;
        }

        self.assembly_ranges.push(DisasmAssemblyRange {
            start_line: line,
            end_line: line,
        });
    }

    fn finish(self) -> DisasmSourceBlock {
        DisasmSourceBlock {
            source: self.source,
            assembly_ranges: self.assembly_ranges,
        }
    }
}

struct DisasmBlockCollector<'a> {
    source_map: &'a SourceMap,
    block_indexes: HashMap<SourceLocationKey, usize>,
    blocks: Vec<DisasmSourceBlockBuilder>,
    current_line: usize,
}

impl<'a> DisasmBlockCollector<'a> {
    fn new(source_map: &'a SourceMap) -> Self {
        Self {
            source_map,
            block_indexes: HashMap::new(),
            blocks: Vec::new(),
            current_line: 0,
        }
    }

    fn collect(mut self, code: &Code, show_offsets: bool) -> Vec<DisasmSourceBlock> {
        if show_offsets {
            self.push_line(None);
            self.push_line(None);
        }

        self.collect_instructions(&code.instructions, code.offsets.as_deref());
        self.blocks
            .into_iter()
            .filter(|block| !block.assembly_ranges.is_empty())
            .map(DisasmSourceBlockBuilder::finish)
            .collect()
    }

    fn collect_instructions(&mut self, instructions: &[Instruction], offsets: Option<&[u16]>) {
        for (index, instruction) in instructions.iter().enumerate() {
            let offset = offsets.and_then(|values| values.get(index).copied());
            self.collect_instruction(instruction, offset);
        }
    }

    fn collect_instruction(&mut self, instruction: &Instruction, offset: Option<u16>) {
        let location = instruction_source_location(self.source_map, instruction, offset);
        self.push_line(location.as_ref());

        match instruction {
            Instruction::Plain(instr) => {
                for arg in &instr.args {
                    self.collect_arg(arg, location.as_ref());
                }
            }
            Instruction::Ref(instr) => self.collect_arg(&instr.code, location.as_ref()),
            Instruction::ExoticCell(_) | Instruction::Slice(_) => {}
        }
    }

    fn collect_arg(&mut self, arg: &ArgValue, container_location: Option<&SourceLocation>) {
        match arg {
            ArgValue::Code { code, .. } => {
                self.collect_instructions(&code.instructions, code.offsets.as_deref());
                self.push_line(container_location);
            }
            ArgValue::CodeDictionary(dict) => {
                for method in &dict.methods {
                    let method_location = first_instruction_location(
                        self.source_map,
                        &method.instructions,
                        method.offsets.as_deref(),
                    )
                    .or_else(|| container_location.cloned());
                    self.push_line(method_location.as_ref());
                    self.collect_instructions(&method.instructions, method.offsets.as_deref());
                    self.push_line(method_location.as_ref());
                }
                self.push_line(container_location);
            }
            _ => {}
        }
    }

    fn push_line(&mut self, location: Option<&SourceLocation>) {
        if let Some(location) = location {
            self.record_line(location, self.current_line);
        }
        self.current_line += 1;
    }

    fn record_line(&mut self, location: &SourceLocation, line: usize) {
        let key = SourceLocationKey::from(location);
        let block_index = if let Some(index) = self.block_indexes.get(&key) {
            *index
        } else {
            let next_index = self.blocks.len();
            self.blocks.push(DisasmSourceBlockBuilder::new(location));
            self.block_indexes.insert(key, next_index);
            next_index
        };

        self.blocks[block_index].add_line(line);
    }
}

fn build_json_output(code: &Code, opts: &FormatOptions) -> DisasmJsonOutput {
    let render_opts = FormatOptions {
        source_map: None,
        ..opts.clone()
    };
    let assembly = code.print(&render_opts);
    let blocks = opts
        .source_map
        .as_deref()
        .map_or_else(Vec::new, |source_map| {
            DisasmBlockCollector::new(source_map).collect(code, opts.show_offsets)
        });

    DisasmJsonOutput {
        success: true,
        assembly,
        blocks,
    }
}

fn write_output_file(output_path: &str, output: &str) -> anyhow::Result<()> {
    if let Some(parent_dir) = Path::new(output_path).parent()
        && let Err(err) = fs::create_dir_all(parent_dir)
    {
        anyhow::bail!(
            "Failed to create output directory {}: {}",
            parent_dir.display(),
            err
        );
    }

    fs::write(output_path, output)?;
    Ok(())
}

fn instruction_source_location(
    source_map: &SourceMap,
    instruction: &Instruction,
    offset: Option<u16>,
) -> Option<SourceLocation> {
    let offset = offset?;
    match instruction {
        Instruction::Plain(instr) => {
            cell_source_location(source_map, instr.source_cell.as_ref(), offset)
        }
        Instruction::Ref(instr) => {
            cell_source_location(source_map, instr.source_cell.as_ref(), offset)
        }
        Instruction::ExoticCell(instr) => {
            cell_source_location(source_map, instr.source_cell.as_ref(), offset)
        }
        Instruction::Slice(instr) => {
            cell_source_location(source_map, instr.source_cell.as_ref(), offset)
        }
    }
}

fn cell_source_location(
    source_map: &SourceMap,
    cell: Option<&Cell>,
    offset: u16,
) -> Option<SourceLocation> {
    let cell = cell?;
    let hash = cell.repr_hash().to_string().to_uppercase();
    source_map.find_source_loc(&hash, offset)
}

fn first_instruction_location(
    source_map: &SourceMap,
    instructions: &[Instruction],
    offsets: Option<&[u16]>,
) -> Option<SourceLocation> {
    for (index, instruction) in instructions.iter().enumerate() {
        let offset = offsets.and_then(|values| values.get(index).copied());
        if let Some(location) = instruction_source_location(source_map, instruction, offset) {
            return Some(location);
        }
    }

    None
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
        Instruction::Plain(_) | Instruction::Ref(_) | Instruction::Slice(_) => None,
    }
}
