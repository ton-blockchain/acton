use acton_debug::RenderedValue;
use acton_debug::replayer::{ExceptionBreakMode, LocalVarRendered, StepMode, Tick, TolkReplayer};
use anyhow::Context;
use base64::Engine as _;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use tolk_source_map::SourceMap;
use tolk_source_map::debug_marks_dict::parse_debug_marks;
use tolk_source_map::source_map::{DebugMark, SrcRange, SymbolTypesJson};
use tycho_types::boc::Boc;

const MAX_SOURCE_TRACE_STEPS: usize = 10_000;
const MAX_RENDERED_VALUE_DEPTH: usize = 2;
const MAX_RENDERED_CHILDREN: usize = 64;
const INTERNAL_MESSAGE_ENTRYPOINT: &str = "onInternalMessage";

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct BuildSourceTraceRequest {
    pub vm_logs: String,
    pub code_hash: String,
    pub source_bundle: SourceTraceBundleRequest,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context: Option<SourceTraceContextRequest>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildCompiledSourceTraceRequest {
    pub vm_logs: String,
    pub code_hash: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context: Option<SourceTraceContextRequest>,
    pub compiled: CompiledTolkSourceTrace,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompiledTolkSourceTrace {
    pub code_boc64: String,
    pub symbol_types_json: SymbolTypesJson,
    pub debug_marks_json: Vec<DebugMark>,
    pub debug_marks_base64: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceTraceContextRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub in_msg: Option<SourceTraceInMessageContextRequest>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceTraceInMessageContextRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sender_address: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SourceTraceBundleRequest {
    pub source_bundle_hash: String,
    pub entrypoint: String,
    pub compiler: SourceTraceCompilerRequest,
    pub files: Vec<SourceTraceFileRequest>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SourceTraceCompilerRequest {
    pub language: String,
    pub version: String,
    #[serde(default)]
    pub params: Value,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SourceTraceFileRequest {
    pub path: String,
    pub content: String,
}

impl SourceTraceBundleRequest {
    #[must_use]
    pub fn import_mappings(&self) -> Option<BTreeMap<String, String>> {
        self.compiler
            .params
            .get("import_mappings")
            .and_then(Value::as_object)
            .map(|mappings| {
                mappings
                    .iter()
                    .filter_map(|(key, value)| {
                        value.as_str().map(|value| (key.clone(), value.to_owned()))
                    })
                    .collect()
            })
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceTraceResponse {
    pub code_hash: String,
    pub files: Vec<SourceTraceFileInfo>,
    pub steps: Vec<SourceTraceStep>,
    pub truncated: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceTraceFileInfo {
    pub path: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceTraceStep {
    pub index: usize,
    pub location: SourceTraceLocation,
    pub instruction: Option<String>,
    pub vm_position: Option<SourceTraceVmPosition>,
    pub locals: Vec<SourceTraceVariable>,
    pub stack: Vec<String>,
    pub call_stack: Vec<SourceTraceFrame>,
    pub exception: Option<SourceTraceException>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceTraceLocation {
    pub file: String,
    pub line: usize,
    pub column: usize,
    pub end_line: usize,
    pub end_column: usize,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceTraceVmPosition {
    pub cell_hash: String,
    pub offset: i32,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceTraceFrame {
    pub function_name: String,
    pub location: Option<SourceTraceLocation>,
    pub is_inlined: bool,
    pub is_builtin: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct SourceTraceVariable {
    pub name: String,
    pub value: String,
    #[serde(rename = "type")]
    pub type_field: Option<String>,
    pub children: Vec<SourceTraceVariable>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceTraceException {
    pub errno: String,
    pub symbolic_name: Option<String>,
    pub is_uncaught: bool,
}

#[derive(Clone, Debug)]
pub struct SourceTracePathRoots {
    pub root: PathBuf,
    pub canonical_root: PathBuf,
}

pub fn validate_bundle(bundle: &SourceTraceBundleRequest) -> anyhow::Result<()> {
    if bundle.compiler.language.trim().to_lowercase() != "tolk" {
        anyhow::bail!("Source-level retrace supports only Tolk bundles");
    }
    if !is_compiler_version_at_least(&bundle.compiler.version, [1, 4, 0]) {
        anyhow::bail!(
            "Source-level retrace requires Tolk compiler 1.4.0 or newer, got {}",
            bundle.compiler.version
        );
    }
    if bundle.files.is_empty() {
        anyhow::bail!("Source bundle does not contain files");
    }
    if bundle.entrypoint.trim().is_empty() {
        anyhow::bail!("Source bundle does not contain entrypoint");
    }
    Ok(())
}

pub fn build_compiled_source_trace_response(
    payload: BuildCompiledSourceTraceRequest,
) -> anyhow::Result<SourceTraceResponse> {
    if payload.compiled.debug_marks_json.is_empty() {
        anyhow::bail!("Compiler did not return debug marks JSON");
    }
    if payload.compiled.debug_marks_base64.trim().is_empty() {
        anyhow::bail!("Compiler did not return debug marks dictionary");
    }

    let marks_dict = parse_debug_marks(
        Some(payload.compiled.debug_marks_base64.as_str()),
        &payload.compiled.code_boc64,
    )?;
    let source_map = SourceMap::from_parts(
        payload.compiled.symbol_types_json,
        payload.compiled.debug_marks_json,
        marks_dict,
    );

    build_source_trace_response(
        &payload.vm_logs,
        &payload.code_hash,
        payload.context.as_ref(),
        &payload.compiled.code_boc64,
        &source_map,
        None,
    )
}

pub fn build_source_trace_response(
    vm_logs: &str,
    code_hash: &str,
    context: Option<&SourceTraceContextRequest>,
    code_boc64: &str,
    source_map: &SourceMap,
    path_roots: Option<&SourceTracePathRoots>,
) -> anyhow::Result<SourceTraceResponse> {
    let expected_code_hash = parse_hash_any(code_hash)?;
    let code_cell = Boc::decode_base64(code_boc64).context("Failed to decode compiled code BoC")?;
    let compiled_hash = code_cell.repr_hash().as_array().to_owned();
    if compiled_hash != expected_code_hash {
        anyhow::bail!(
            "Verified source bundle code hash mismatch: expected {}, compiled {}",
            hash_to_hex(&expected_code_hash),
            hash_to_hex(&compiled_hash)
        );
    }

    if source_map.debug_marks_count() == 0 {
        anyhow::bail!("Compiler did not return debug marks JSON");
    }
    if !source_map.has_debug_marks() {
        anyhow::bail!("Compiler did not return debug marks dictionary");
    }

    let source_path_by_file_id = source_path_by_file_id(source_map, path_roots);

    let mut replayer = TolkReplayer::new(source_map, vm_logs)?;
    replayer.set_exception_breakpoints(ExceptionBreakMode::Uncaught);

    let mut steps = Vec::new();
    let mut truncated = false;

    while !replayer.is_finished() {
        if steps.len() >= MAX_SOURCE_TRACE_STEPS {
            truncated = true;
            break;
        }

        let mut instruction = None;
        replayer.step_with_callback(StepMode::StepInto, |tick, _replayer| {
            if let Tick::TvmAfterExecute { instr_name } = tick {
                instruction = Some(instr_name.clone());
            }
        });

        let Some(location) = current_location(&replayer, &source_path_by_file_id) else {
            continue;
        };
        let call_stack = source_trace_call_stack(&replayer, &location, &source_path_by_file_id);
        let mut locals: Vec<_> = replayer
            .locals_for_frame(0)
            .into_iter()
            .map(source_trace_variable)
            .collect();
        inject_context_variables(context, &call_stack, &mut locals);

        steps.push(SourceTraceStep {
            index: steps.len(),
            location,
            instruction,
            vm_position: replayer.current_vm_position().map(|(cell_hash, offset)| {
                SourceTraceVmPosition {
                    cell_hash: cell_hash.to_owned(),
                    offset,
                }
            }),
            locals,
            stack: replayer.tvm_stack_rendered(),
            call_stack,
            exception: replayer
                .last_exception()
                .map(|exception| SourceTraceException {
                    errno: exception.errno.clone(),
                    symbolic_name: exception.symbolic_name.clone(),
                    is_uncaught: exception.is_uncaught,
                }),
        });
    }

    Ok(SourceTraceResponse {
        code_hash: hash_to_hex(&expected_code_hash),
        files: source_path_by_file_id
            .values()
            .map(|path| SourceTraceFileInfo { path: path.clone() })
            .collect(),
        steps,
        truncated,
    })
}

fn inject_context_variables(
    context: Option<&SourceTraceContextRequest>,
    call_stack: &[SourceTraceFrame],
    locals: &mut Vec<SourceTraceVariable>,
) {
    if call_stack
        .first()
        .is_none_or(|frame| frame.function_name != INTERNAL_MESSAGE_ENTRYPOINT)
    {
        return;
    }

    let Some(sender_address) = context
        .and_then(|context| context.in_msg.as_ref())
        .and_then(|in_msg| in_msg.sender_address.as_deref())
        .filter(|sender_address| !sender_address.is_empty())
    else {
        return;
    };

    let sender_address = LocalVarRendered::in_sender_address(sender_address);
    if locals
        .iter()
        .any(|local| local.name == sender_address.var_name)
    {
        return;
    }

    locals.insert(0, source_trace_variable(sender_address));
}

fn is_compiler_version_at_least(version: &str, minimum: [u64; 3]) -> bool {
    let parsed = parse_compiler_version(version);
    for (current, minimum) in parsed.into_iter().zip(minimum) {
        if current > minimum {
            return true;
        }
        if current < minimum {
            return false;
        }
    }
    true
}

fn parse_compiler_version(version: &str) -> [u64; 3] {
    let mut result = [0, 0, 0];
    for (index, part) in version
        .split(|ch: char| !ch.is_ascii_digit())
        .filter(|part| !part.is_empty())
        .take(3)
        .enumerate()
    {
        result[index] = part.parse().unwrap_or(0);
    }
    result
}

fn source_path_by_file_id(
    source_map: &SourceMap,
    path_roots: Option<&SourceTracePathRoots>,
) -> BTreeMap<usize, String> {
    source_map
        .files()
        .iter()
        .filter(|file| should_show_source_file(&file.file_name))
        .map(|file| {
            let path = source_map_file_path(path_roots, &file.file_name);
            (file.file_id, path)
        })
        .collect()
}

fn source_map_file_path(path_roots: Option<&SourceTracePathRoots>, path: &str) -> String {
    let path = Path::new(path);
    if let Some(path_roots) = path_roots {
        for root in [&path_roots.root, &path_roots.canonical_root] {
            if let Ok(relative_path) = path.strip_prefix(root) {
                return normalize_source_path(&relative_path.to_string_lossy());
            }
        }
    }
    normalize_source_path(&path.to_string_lossy())
}

fn current_location(
    replayer: &TolkReplayer,
    source_path_by_file_id: &BTreeMap<usize, String>,
) -> Option<SourceTraceLocation> {
    let line = replayer.current_line();
    let column = replayer.current_column();
    if line == 0 {
        return None;
    }
    let file = source_path_by_file_id
        .get(&replayer.current_file_id())?
        .to_owned();

    Some(SourceTraceLocation {
        file,
        line,
        column,
        end_line: replayer.current_end_line(),
        end_column: replayer.current_end_column(),
    })
}

fn range_location(
    range: &SrcRange,
    source_path_by_file_id: &BTreeMap<usize, String>,
) -> Option<SourceTraceLocation> {
    let line = range.start_line();
    let column = range.start_col();
    if line == 0 && column == 0 {
        return None;
    }
    let file = source_path_by_file_id.get(&range.file_id())?.to_owned();

    Some(SourceTraceLocation {
        file,
        line,
        column,
        end_line: range.end_line(),
        end_column: range.end_col(),
    })
}

fn source_trace_call_stack(
    replayer: &TolkReplayer,
    current_location: &SourceTraceLocation,
    source_path_by_file_id: &BTreeMap<usize, String>,
) -> Vec<SourceTraceFrame> {
    let frames = replayer.call_stack();
    let frame_count = frames.len();

    (0..frame_count)
        .map(|depth| {
            let frame_index = frame_count - 1 - depth;
            let frame = &frames[frame_index];
            let location = if depth == 0 {
                Some(current_location.clone())
            } else {
                frames
                    .get(frame_index + 1)
                    .and_then(|child_frame| child_frame.call_site_loc.as_ref())
                    .and_then(|range| range_location(range, source_path_by_file_id))
            };

            SourceTraceFrame {
                function_name: frame.f_name.clone(),
                location,
                is_inlined: frame.is_inlined,
                is_builtin: frame.is_builtin,
            }
        })
        .collect()
}

fn source_trace_variable(local: LocalVarRendered) -> SourceTraceVariable {
    rendered_value_variable(local.var_name, &local.value, 0)
}

fn rendered_value_variable(
    name: String,
    value: &RenderedValue,
    depth: usize,
) -> SourceTraceVariable {
    let (display_value, type_field) = value.dap_parts_for_client(Some(&name));
    let children = if depth >= MAX_RENDERED_VALUE_DEPTH {
        Vec::new()
    } else {
        rendered_value_children(value)
            .into_iter()
            .take(MAX_RENDERED_CHILDREN)
            .map(|(name, value)| rendered_value_variable(name, value, depth + 1))
            .collect()
    };

    SourceTraceVariable {
        name,
        value: display_value,
        type_field,
        children,
    }
}

fn rendered_value_children(value: &RenderedValue) -> Vec<(String, &RenderedValue)> {
    match value {
        RenderedValue::Struct { fields, .. }
        | RenderedValue::MapKV { fields, .. }
        | RenderedValue::Address { fields, .. }
        | RenderedValue::CellLike { fields, .. }
        | RenderedValue::CellOf { fields, .. }
        | RenderedValue::EnumValue { fields, .. }
        | RenderedValue::UnionCase { fields, .. } => fields
            .iter()
            .map(|(name, value)| (name.clone(), value))
            .collect(),
        RenderedValue::Tensor { items, .. } | RenderedValue::ArrayOf { items, .. } => items
            .iter()
            .enumerate()
            .map(|(index, value)| (index.to_string(), value))
            .collect(),
        RenderedValue::LastSeen { inner } => rendered_value_children(inner),
        RenderedValue::LazyNotYetLoaded { preview } => rendered_value_children(preview),
        RenderedValue::Leaf { .. }
        | RenderedValue::OptimizedOut
        | RenderedValue::LazyCantParseSlice
        | RenderedValue::LazyUnresolved { .. } => Vec::new(),
    }
}

fn should_show_source_file(path: &str) -> bool {
    !normalize_source_path(path)
        .to_lowercase()
        .ends_with(".abi.json")
}

fn normalize_source_path(path: &str) -> String {
    path.replace('\\', "/").trim_start_matches("./").to_owned()
}

fn parse_hash_any(hash: &str) -> anyhow::Result<[u8; 32]> {
    let trimmed = hash.trim();
    if let Ok(bytes) = hex::decode(trimmed)
        && bytes.len() == 32
    {
        return bytes
            .try_into()
            .map_err(|_| anyhow::anyhow!("Invalid hash format"));
    }

    if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(trimmed)
        && bytes.len() == 32
    {
        return bytes
            .try_into()
            .map_err(|_| anyhow::anyhow!("Invalid hash format"));
    }

    anyhow::bail!("Invalid hash format")
}

fn hash_to_hex(hash: &[u8; 32]) -> String {
    hex::encode(hash)
}
