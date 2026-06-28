use super::utils::handle_result;
use crate::server::models::{
    BuildSourceTraceRequest, SourceTraceBundleRequest, SourceTraceFileRequest,
};
use acton_source_trace::{
    SourceTracePathRoots, SourceTraceResponse,
    build_source_trace_response as build_source_trace_from_source_map, validate_bundle,
};
use anyhow::Context;
use axum::Json;
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process;
use std::time::{SystemTime, UNIX_EPOCH};
use tolk_compiler::{Compiler, CompilerResult};

struct SourceTraceTempDir {
    root: PathBuf,
    canonical_root: PathBuf,
}

impl Drop for SourceTraceTempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
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

    let source_map = compiled
        .source_map
        .context("Compiler did not return source map")?;
    let path_roots = SourceTracePathRoots {
        root: temp_dir.root.clone(),
        canonical_root: temp_dir.canonical_root.clone(),
    };
    build_source_trace_from_source_map(
        &payload.vm_logs,
        &payload.code_hash,
        payload.context.as_ref(),
        &compiled.code_boc64,
        &source_map,
        Some(&path_roots),
    )
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

    fs::write(&path, file.content.as_bytes())
        .with_context(|| format!("Failed to write {}", path.display()))
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

fn normalize_source_path(path: &str) -> String {
    path.replace('\\', "/").trim_start_matches("./").to_owned()
}
