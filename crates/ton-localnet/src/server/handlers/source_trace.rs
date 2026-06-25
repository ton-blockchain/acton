use super::utils::handle_result;
use crate::server::models::{
    BuildSourceTraceRequest, SourceTraceBundleRequest, SourceTraceFileRequest,
};
use crate::types::Hash256;
use acton_debug::RenderedValue;
use acton_debug::replayer::{ExceptionBreakMode, LocalVarRendered, StepMode, Tick, TolkReplayer};
use anyhow::Context;
use axum::Json;
use base64::Engine;
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process;
use std::time::{SystemTime, UNIX_EPOCH};
use tolk_compiler::{Compiler, CompilerResult, SourceMap, source_map::SrcRange};
use tycho_types::boc::Boc;

const MAX_SOURCE_TRACE_STEPS: usize = 10_000;
const MAX_RENDERED_VALUE_DEPTH: usize = 2;
const MAX_RENDERED_CHILDREN: usize = 64;

struct SourceTraceTempDir {
    root: PathBuf,
    canonical_root: PathBuf,
}

impl Drop for SourceTraceTempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[derive(Serialize)]
struct SourceTraceResponse {
    source_bundle_hash: String,
    code_hash: String,
    entrypoint: String,
    files: Vec<SourceTraceFileInfo>,
    steps: Vec<SourceTraceStep>,
    truncated: bool,
}

#[derive(Serialize)]
struct SourceTraceFileInfo {
    path: String,
    is_entrypoint: bool,
}

#[derive(Serialize)]
struct SourceTraceStep {
    index: usize,
    location: SourceTraceLocation,
    instruction: Option<String>,
    vm_position: Option<SourceTraceVmPosition>,
    locals: Vec<SourceTraceVariable>,
    stack: Vec<String>,
    call_stack: Vec<SourceTraceFrame>,
    exception: Option<SourceTraceException>,
}

#[derive(Clone, Serialize)]
struct SourceTraceLocation {
    file: String,
    line: usize,
    column: usize,
    end_line: usize,
    end_column: usize,
}

#[derive(Serialize)]
struct SourceTraceVmPosition {
    cell_hash: String,
    offset: i32,
}

#[derive(Serialize)]
struct SourceTraceFrame {
    function_name: String,
    location: Option<SourceTraceLocation>,
    is_inlined: bool,
    is_builtin: bool,
}

#[derive(Serialize)]
struct SourceTraceVariable {
    name: String,
    value: String,
    #[serde(rename = "type")]
    type_field: Option<String>,
    children: Vec<SourceTraceVariable>,
}

#[derive(Serialize)]
struct SourceTraceException {
    errno: String,
    symbolic_name: Option<String>,
    is_uncaught: bool,
}

pub async fn build_source_trace(Json(payload): Json<BuildSourceTraceRequest>) -> Json<Value> {
    handle_result(
        async move {
            tokio::task::spawn_blocking(move || build_source_trace_response(payload))
                .await
                .context("Source trace builder task failed")?
        },
        |response| serde_json::to_value(response).unwrap_or(Value::Null),
    )
    .await
}

fn build_source_trace_response(
    payload: BuildSourceTraceRequest,
) -> anyhow::Result<SourceTraceResponse> {
    validate_bundle(&payload.source_bundle)?;

    let expected_code_hash = parse_hash_any(&payload.code_hash)?;
    let temp_dir = write_source_bundle(&payload.source_bundle)?;
    let entrypoint_path = temp_dir
        .root
        .join(safe_relative_path(&payload.source_bundle.entrypoint)?);
    let mappings = temp_import_mappings(&temp_dir.root, &payload.source_bundle)?;
    let compiler = Compiler::new(2).with_mappings(&Some(mappings));
    let compilation_result = compiler.compile(&entrypoint_path, true);

    let compiled = match compilation_result {
        CompilerResult::Success(result) => result,
        CompilerResult::Error(error) => {
            anyhow::bail!(
                "Failed to compile verified source bundle {}: {}",
                payload.source_bundle.source_bundle_hash,
                error.message.trim_end()
            );
        }
    };

    let code_cell =
        Boc::decode_base64(compiled.code_boc64).context("Failed to decode compiled code BoC")?;
    let compiled_hash = Hash256::from(code_cell.repr_hash());
    if compiled_hash != expected_code_hash {
        anyhow::bail!(
            "Verified source bundle code hash mismatch: expected {}, compiled {}",
            expected_code_hash.to_hex(),
            compiled_hash.to_hex()
        );
    }

    let source_map = compiled
        .source_map
        .context("Compiler did not return source map")?;
    if !source_map.has_debug_marks() {
        anyhow::bail!("Compiler did not return debug marks");
    }
    let source_path_by_file_id =
        source_path_by_file_id(&source_map, &temp_dir, &payload.source_bundle);

    let mut replayer = TolkReplayer::new(&source_map, &payload.vm_logs)?;
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
            locals: replayer
                .locals_for_frame(0)
                .into_iter()
                .map(source_trace_variable)
                .collect(),
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
        source_bundle_hash: payload.source_bundle.source_bundle_hash.clone(),
        code_hash: expected_code_hash.to_hex(),
        entrypoint: normalize_source_path(&payload.source_bundle.entrypoint),
        files: payload
            .source_bundle
            .files
            .iter()
            .filter(|file| should_show_source_file(&file.path))
            .map(|file| SourceTraceFileInfo {
                path: normalize_source_path(&file.path),
                is_entrypoint: same_source_path(&file.path, &payload.source_bundle.entrypoint),
            })
            .collect(),
        steps,
        truncated,
    })
}

fn validate_bundle(bundle: &SourceTraceBundleRequest) -> anyhow::Result<()> {
    if bundle.language.trim().to_lowercase() != "tolk" {
        anyhow::bail!("Source-level retrace supports only Tolk bundles");
    }
    if !is_compiler_version_at_least(&bundle.compiler_version, [1, 4, 0]) {
        anyhow::bail!(
            "Source-level retrace requires Tolk compiler 1.4.0 or newer, got {}",
            bundle.compiler_version
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

fn write_source_bundle(bundle: &SourceTraceBundleRequest) -> anyhow::Result<SourceTraceTempDir> {
    let root = source_trace_temp_root(&bundle.source_bundle_hash)?;
    fs::create_dir_all(&root)
        .with_context(|| format!("Failed to create source trace temp dir {}", root.display()))?;

    let canonical_root = dunce::canonicalize(&root).unwrap_or_else(|_| root.clone());
    let temp_dir = SourceTraceTempDir {
        root,
        canonical_root,
    };
    for file in &bundle.files {
        write_source_file(&temp_dir.root, file)?;
    }

    Ok(temp_dir)
}

fn source_trace_temp_root(bundle_hash: &str) -> anyhow::Result<PathBuf> {
    let safe_hash: String = bundle_hash
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .take(24)
        .collect();
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    Ok(std::env::temp_dir().join(format!(
        "acton-localnet-source-trace-{}-{}-{now}",
        process::id(),
        if safe_hash.is_empty() {
            "bundle"
        } else {
            safe_hash.as_str()
        },
    )))
}

fn write_source_file(root: &Path, file: &SourceTraceFileRequest) -> anyhow::Result<()> {
    let path = root.join(safe_relative_path(&file.path)?);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create source dir {}", parent.display()))?;
    }

    let content = source_file_content(file)?;
    fs::write(&path, content).with_context(|| format!("Failed to write {}", path.display()))
}

fn source_file_content(file: &SourceTraceFileRequest) -> anyhow::Result<Vec<u8>> {
    if let Some(content) = &file.content_text {
        return Ok(content.as_bytes().to_vec());
    }

    base64::engine::general_purpose::STANDARD
        .decode(file.content_base64.as_bytes())
        .with_context(|| format!("Failed to decode {}", file.path))
}

fn temp_import_mappings(
    root: &Path,
    bundle: &SourceTraceBundleRequest,
) -> anyhow::Result<BTreeMap<String, String>> {
    let mut mappings = BTreeMap::new();

    for (key, value) in bundle.import_mappings().unwrap_or_default() {
        let path = root.join(safe_relative_path(&value)?);
        mappings.insert(key, path.to_string_lossy().into_owned());
    }

    Ok(mappings)
}

fn safe_relative_path(path: &str) -> anyhow::Result<PathBuf> {
    let normalized = normalize_source_path(path);
    let mut result = PathBuf::new();

    for component in Path::new(&normalized).components() {
        match component {
            Component::Normal(part) => result.push(part),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                anyhow::bail!("Unsafe source path `{path}`")
            }
        }
    }

    if result.as_os_str().is_empty() {
        anyhow::bail!("Empty source path");
    }

    Ok(result)
}

fn source_path_by_file_id(
    source_map: &SourceMap,
    temp_dir: &SourceTraceTempDir,
    bundle: &SourceTraceBundleRequest,
) -> BTreeMap<usize, String> {
    let bundle_paths: BTreeMap<String, String> = bundle
        .files
        .iter()
        .filter(|file| should_show_source_file(&file.path))
        .map(|file| {
            let path = normalize_source_path(&file.path);
            (path.clone(), path)
        })
        .collect();

    source_map
        .files()
        .iter()
        .filter_map(|file| {
            let path = source_map_file_path(temp_dir, &file.file_name);
            bundle_paths
                .get(&path)
                .cloned()
                .map(|path| (file.file_id, path))
        })
        .collect()
}

fn source_map_file_path(temp_dir: &SourceTraceTempDir, path: &str) -> String {
    let path = Path::new(path);
    for root in [&temp_dir.root, &temp_dir.canonical_root] {
        if let Ok(relative_path) = path.strip_prefix(root) {
            return normalize_source_path(&relative_path.to_string_lossy());
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
    let (display_value, type_field) = value.dap_parts();
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

fn same_source_path(left: &str, right: &str) -> bool {
    normalize_source_path(left) == normalize_source_path(right)
}

fn normalize_source_path(path: &str) -> String {
    path.replace('\\', "/").trim_start_matches("./").to_owned()
}

fn parse_hash_any(hash: &str) -> anyhow::Result<Hash256> {
    if let Ok(parsed) = Hash256::from_hex(hash) {
        return Ok(parsed);
    }
    if let Ok(parsed) = Hash256::from_base64(hash) {
        return Ok(parsed);
    }
    anyhow::bail!("Invalid hash format")
}
