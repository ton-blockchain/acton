use crate::contract_interface::is_boc_path;
use acton_config::color::OwoColorize;
use acton_config::config::{ActonConfig, ContractConfig};
use anyhow::{Context, anyhow};
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Component, Path, PathBuf};
use tolk_compiler::SourceMap;
use tolk_compiler::abi::ContractABI;
use tolk_compiler::source_map::{DebugMark, SymbolTypesJson};

use super::contract_artifact_path;

const TOLK_SOURCE_BUNDLE_SCHEMA_VERSION: u8 = 1;

#[derive(Debug, Clone)]
pub(super) struct SourceArtifactDebugInfo {
    code_boc64: String,
    symbol_types_json: SymbolTypesJson,
    debug_marks_json: Vec<DebugMark>,
    debug_marks_base64: String,
}

impl SourceArtifactDebugInfo {
    pub(super) fn from_compiler_result(
        code_boc64: &str,
        symbol_types_json: Option<SymbolTypesJson>,
        debug_marks_json: Option<Vec<DebugMark>>,
        debug_marks_base64: Option<String>,
    ) -> Option<Self> {
        Some(Self {
            code_boc64: code_boc64.to_owned(),
            symbol_types_json: symbol_types_json?,
            debug_marks_json: debug_marks_json?,
            debug_marks_base64: debug_marks_base64?.trim().to_owned(),
        })
        .filter(|debug_info| !debug_info.debug_marks_base64.is_empty())
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn save_source_artifact(
    project_root: &Path,
    output_sources_dir: &Path,
    contract_key: &str,
    contract_config: &ContractConfig,
    contract_path: &Path,
    code_hash: &str,
    source_map: Option<&SourceMap>,
    debug_info: Option<&SourceArtifactDebugInfo>,
    abi: Option<&ContractABI>,
    config: &ActonConfig,
) -> anyhow::Result<()> {
    if is_boc_path(contract_path) {
        anyhow::bail!(
            "Cannot save source artifact for precompiled contract {}. Source artifacts require a .tolk contract source.",
            contract_config.display_name(contract_key).yellow()
        );
    }

    let source_map = source_map.ok_or_else(|| {
        anyhow!(
            "Cannot save source artifact for {} because the compiler did not return source maps",
            contract_config.display_name(contract_key).yellow()
        )
    })?;
    let debug_info = debug_info.ok_or_else(|| {
        anyhow!(
            "Cannot save source artifact for {} because the compiler did not return debug info",
            contract_config.display_name(contract_key).yellow()
        )
    })?;
    let mut files = source_artifact_files(source_map, project_root)?;
    let entrypoint = files
        .iter()
        .find(|file| file.is_entrypoint)
        .map(|file| file.path.clone())
        .ok_or_else(|| {
            anyhow!(
                "Cannot save source artifact for {} because the entrypoint source was not found",
                contract_config.display_name(contract_key).yellow()
            )
        })?;

    if let Some(abi) = abi {
        files.push(generated_abi_source_file(&entrypoint, abi)?);
    }
    files.sort_by(|left, right| left.path.cmp(&right.path));

    let compiler_version = tolk_compiler::native_tolk_version()
        .context("Failed to read native Tolk compiler version")?
        .version;
    let compiler_params = source_artifact_compiler_params(config, project_root, &compiler_version)?;
    let bundle_hash = compute_source_bundle_hash(SourceBundleHashInput {
        compiler: SourceBundleHashCompiler {
            language: "tolk",
            version: &compiler_version,
            entrypoint: &entrypoint,
            params: &compiler_params,
        },
        sources: files
            .iter()
            .map(|file| SourceBundleHashSource {
                path: file.path.as_str(),
                include_in_command: file.include_in_command,
                is_stdlib: file.is_stdlib,
                has_include_directives: file.has_include_directives,
            })
            .collect(),
        files: files
            .iter()
            .map(|file| SourceBundleHashFile {
                path: file.path.as_str(),
                bytes: file.content.as_bytes(),
            })
            .collect(),
    })?;

    let artifact = SourceRegistrationArtifact {
        code_hash: code_hash.to_owned(),
        verified: true,
        bundles: vec![SourceRegistrationBundle {
            source_bundle_hash: bundle_hash,
            verified_at: 0,
            storage_revision: "local".to_owned(),
            entrypoint,
            compiler: SourceRegistrationCompiler {
                language: "tolk".to_owned(),
                version: compiler_version,
                params: compiler_params,
            },
            compiler_abi: abi.cloned(),
            source_map: Some(SourceRegistrationSourceMap {
                code_boc64: debug_info.code_boc64.clone(),
                symbol_types_json: normalized_symbol_types_json(
                    &debug_info.symbol_types_json,
                    project_root,
                )?,
                debug_marks_json: debug_info.debug_marks_json.clone(),
                debug_marks_base64: debug_info.debug_marks_base64.clone(),
            }),
            files: files
                .into_iter()
                .map(|file| SourceRegistrationFile {
                    path: file.path,
                    content_hash: file.content_hash,
                    include_in_command: file.include_in_command,
                    is_stdlib: file.is_stdlib,
                    has_include_directives: file.has_include_directives,
                    content: file.content,
                })
                .collect(),
        }],
    };

    let path = contract_artifact_path(output_sources_dir, contract_key, "source.json");
    if let Some(parent_dir) = path.parent()
        && let Err(err) = fs::create_dir_all(parent_dir)
    {
        anyhow::bail!(
            "Failed to create directory for source artifact file {}: {}",
            parent_dir.display(),
            err
        );
    }

    let display_path = path.strip_prefix(project_root).unwrap_or(&path);
    fs::write(&path, serde_json::to_string(&artifact)?).map_err(|err| {
        anyhow!(
            "Failed to save source artifact file {}: {}",
            display_path.display(),
            err
        )
    })?;

    Ok(())
}

fn source_artifact_files(
    source_map: &SourceMap,
    project_root: &Path,
) -> anyhow::Result<Vec<SourceArtifactFile>> {
    source_map
        .files()
        .iter()
        .filter_map(|file| {
            let path = file.file_name.as_str();
            if path.starts_with("@stdlib/") || path.starts_with("@fiftlib/") {
                None
            } else {
                Some((PathBuf::from(path), file.file_id == 1))
            }
        })
        .map(|(path, is_entrypoint)| {
            let source_file_path = if path.is_absolute() {
                path
            } else {
                project_root.join(path)
            };
            let canonical_path = dunce::canonicalize(&source_file_path).unwrap_or(source_file_path);
            let source_path =
                normalize_source_artifact_path(&canonical_path, project_root, "source path")?;
            let content = fs::read_to_string(&canonical_path).with_context(|| {
                format!("Failed to read source file {}", canonical_path.display())
            })?;
            Ok(SourceArtifactFile {
                path: source_path,
                content_hash: sha256_hex(content.as_bytes()),
                include_in_command: Some(true),
                is_stdlib: Some(false),
                has_include_directives: Some(true),
                content,
                is_entrypoint,
            })
        })
        .collect()
}

fn generated_abi_source_file(
    entrypoint: &str,
    abi: &ContractABI,
) -> anyhow::Result<SourceArtifactFile> {
    let path = generated_abi_path(entrypoint);
    let content = format!("{}\n", serde_json::to_string(abi)?);
    Ok(SourceArtifactFile {
        path,
        content_hash: sha256_hex(content.as_bytes()),
        include_in_command: None,
        is_stdlib: None,
        has_include_directives: None,
        content,
        is_entrypoint: false,
    })
}

fn generated_abi_path(entrypoint: &str) -> String {
    let stem = Path::new(entrypoint)
        .file_stem()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .unwrap_or("contract");
    format!("output/{stem}.abi.json")
}

fn normalized_symbol_types_json(
    symbol_types_json: &SymbolTypesJson,
    project_root: &Path,
) -> anyhow::Result<Value> {
    let mut value = serde_json::to_value(symbol_types_json)?;
    let Some(files) = value.get_mut("files").and_then(Value::as_array_mut) else {
        return Ok(value);
    };

    for file in files {
        let Some(file_name_value) = file.get_mut("file_name") else {
            continue;
        };
        let Some(file_name) = file_name_value.as_str() else {
            continue;
        };
        *file_name_value = Value::String(normalize_source_map_file_name(file_name, project_root)?);
    }

    Ok(value)
}

fn normalize_source_map_file_name(file_name: &str, project_root: &Path) -> anyhow::Result<String> {
    if file_name.starts_with("@stdlib/") || file_name.starts_with("@fiftlib/") {
        return Ok(file_name.to_owned());
    }

    let path = Path::new(file_name);
    let normalized_path = if path.is_absolute() {
        dunce::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
    } else {
        path.to_path_buf()
    };
    normalize_source_artifact_path(&normalized_path, project_root, "source map file")
}

fn source_artifact_compiler_params(
    config: &ActonConfig,
    project_root: &Path,
    compiler_version: &str,
) -> anyhow::Result<Value> {
    let mut params = serde_json::Map::new();
    params.insert(
        "compiler_version".to_owned(),
        Value::String(compiler_version.to_owned()),
    );

    let import_mappings = source_artifact_import_mappings(config, project_root)?;
    if !import_mappings.is_empty() {
        params.insert(
            "import_mappings".to_owned(),
            serde_json::to_value(import_mappings)?,
        );
    }

    Ok(Value::Object(params))
}

fn source_artifact_import_mappings(
    config: &ActonConfig,
    project_root: &Path,
) -> anyhow::Result<BTreeMap<String, String>> {
    config
        .mappings
        .clone()
        .unwrap_or_default()
        .into_iter()
        .map(|(key, value)| {
            let normalized_key = if key.starts_with('@') {
                key
            } else {
                format!("@{key}")
            };
            let normalized_value =
                normalize_source_artifact_path(Path::new(&value), project_root, "import mapping")?;
            Ok((normalized_key, normalized_value))
        })
        .collect()
}

fn normalize_source_artifact_path(
    path: &Path,
    project_root: &Path,
    label: &str,
) -> anyhow::Result<String> {
    let relative = if path.is_absolute() {
        path.strip_prefix(project_root).with_context(|| {
            format!(
                "{} must be relative or inside project root: {}",
                label,
                path.display()
            )
        })?
    } else {
        path
    };

    let mut parts = Vec::new();
    for component in relative.components() {
        match component {
            Component::Normal(part) => parts.push(part.to_string_lossy().to_string()),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                anyhow::bail!("{label} contains an invalid component: {}", path.display());
            }
        }
    }

    if parts.is_empty() {
        anyhow::bail!("{label} is empty after normalization: {}", path.display());
    }

    Ok(parts.join("/"))
}

fn compute_source_bundle_hash(input: SourceBundleHashInput<'_>) -> anyhow::Result<String> {
    let canonical = CanonicalSourceBundle::from_input(input);
    let bytes = serde_json::to_vec(&canonical)?;
    Ok(sha256_hex(&bytes))
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

struct SourceArtifactFile {
    path: String,
    content_hash: String,
    include_in_command: Option<bool>,
    is_stdlib: Option<bool>,
    has_include_directives: Option<bool>,
    content: String,
    is_entrypoint: bool,
}

struct SourceBundleHashInput<'a> {
    compiler: SourceBundleHashCompiler<'a>,
    sources: Vec<SourceBundleHashSource<'a>>,
    files: Vec<SourceBundleHashFile<'a>>,
}

struct SourceBundleHashCompiler<'a> {
    language: &'a str,
    version: &'a str,
    entrypoint: &'a str,
    params: &'a Value,
}

struct SourceBundleHashSource<'a> {
    path: &'a str,
    include_in_command: Option<bool>,
    is_stdlib: Option<bool>,
    has_include_directives: Option<bool>,
}

struct SourceBundleHashFile<'a> {
    path: &'a str,
    bytes: &'a [u8],
}

#[derive(Serialize)]
struct CanonicalSourceBundle {
    schema_version: u8,
    compiler: CanonicalSourceBundleCompiler,
    sources: Vec<CanonicalSourceBundleSource>,
    files: Vec<CanonicalSourceBundleFile>,
}

impl CanonicalSourceBundle {
    fn from_input(input: SourceBundleHashInput<'_>) -> Self {
        let mut sources = input
            .sources
            .into_iter()
            .map(|source| CanonicalSourceBundleSource {
                path: source.path.to_owned(),
                include_in_command: source.include_in_command,
                is_stdlib: source.is_stdlib,
                has_include_directives: source.has_include_directives,
            })
            .collect::<Vec<_>>();
        sources.sort_by(|left, right| left.path.cmp(&right.path));

        let mut files = input
            .files
            .into_iter()
            .map(|file| CanonicalSourceBundleFile {
                path: file.path.to_owned(),
                content_hash: sha256_hex(file.bytes),
            })
            .collect::<Vec<_>>();
        files.sort_by(|left, right| left.path.cmp(&right.path));

        Self {
            schema_version: TOLK_SOURCE_BUNDLE_SCHEMA_VERSION,
            compiler: CanonicalSourceBundleCompiler {
                language: input.compiler.language.to_owned(),
                version: input.compiler.version.to_owned(),
                entrypoint: input.compiler.entrypoint.to_owned(),
                params: CanonicalJson::from(input.compiler.params),
            },
            sources,
            files,
        }
    }
}

#[derive(Serialize)]
struct CanonicalSourceBundleCompiler {
    language: String,
    version: String,
    entrypoint: String,
    params: CanonicalJson,
}

#[derive(Serialize)]
struct CanonicalSourceBundleSource {
    path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    include_in_command: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    is_stdlib: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    has_include_directives: Option<bool>,
}

#[derive(Serialize)]
struct CanonicalSourceBundleFile {
    path: String,
    content_hash: String,
}

#[derive(Serialize)]
#[serde(untagged)]
enum CanonicalJson {
    Null,
    Bool(bool),
    Number(serde_json::Number),
    String(String),
    Array(Vec<Self>),
    Object(BTreeMap<String, Self>),
}

impl From<&Value> for CanonicalJson {
    fn from(value: &Value) -> Self {
        match value {
            Value::Null => Self::Null,
            Value::Bool(value) => Self::Bool(*value),
            Value::Number(value) => Self::Number(value.clone()),
            Value::String(value) => Self::String(value.clone()),
            Value::Array(values) => Self::Array(values.iter().map(Self::from).collect()),
            Value::Object(values) => Self::Object(
                values
                    .iter()
                    .map(|(key, value)| (key.clone(), Self::from(value)))
                    .collect(),
            ),
        }
    }
}

#[derive(Serialize)]
struct SourceRegistrationArtifact {
    code_hash: String,
    verified: bool,
    bundles: Vec<SourceRegistrationBundle>,
}

#[derive(Serialize)]
struct SourceRegistrationBundle {
    source_bundle_hash: String,
    verified_at: u64,
    storage_revision: String,
    entrypoint: String,
    compiler: SourceRegistrationCompiler,
    #[serde(skip_serializing_if = "Option::is_none")]
    compiler_abi: Option<ContractABI>,
    #[serde(skip_serializing_if = "Option::is_none")]
    source_map: Option<SourceRegistrationSourceMap>,
    files: Vec<SourceRegistrationFile>,
}

#[derive(Serialize)]
struct SourceRegistrationSourceMap {
    code_boc64: String,
    symbol_types_json: Value,
    debug_marks_json: Vec<DebugMark>,
    debug_marks_base64: String,
}

#[derive(Serialize)]
struct SourceRegistrationCompiler {
    language: String,
    version: String,
    params: Value,
}

#[derive(Serialize)]
struct SourceRegistrationFile {
    path: String,
    content_hash: String,
    include_in_command: Option<bool>,
    is_stdlib: Option<bool>,
    has_include_directives: Option<bool>,
    content: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn generated_abi_path_uses_entrypoint_stem() {
        assert_eq!(
            generated_abi_path("contracts/wallet.tolk"),
            "output/wallet.abi.json"
        );
        assert_eq!(generated_abi_path("wallet"), "output/wallet.abi.json");
    }

    #[test]
    fn source_bundle_hash_sorts_sources_files_and_params() {
        let params = json!({
            "import_mappings": {
                "@stdlib": "stdlib",
                "@lib": "contracts/lib"
            },
            "compiler_version": "1.4.2"
        });

        let hash = compute_source_bundle_hash(SourceBundleHashInput {
            compiler: SourceBundleHashCompiler {
                language: "tolk",
                version: "1.4.2",
                entrypoint: "contracts/main.tolk",
                params: &params,
            },
            sources: vec![
                SourceBundleHashSource {
                    path: "contracts/lib/helper.tolk",
                    include_in_command: Some(true),
                    is_stdlib: Some(false),
                    has_include_directives: Some(false),
                },
                SourceBundleHashSource {
                    path: "contracts/main.tolk",
                    include_in_command: Some(true),
                    is_stdlib: Some(false),
                    has_include_directives: Some(true),
                },
            ],
            files: vec![
                SourceBundleHashFile {
                    path: "contracts/main.tolk",
                    bytes: b"fun main() {}\n",
                },
                SourceBundleHashFile {
                    path: "contracts/lib/helper.tolk",
                    bytes: b"const VALUE = 1;\n",
                },
            ],
        })
        .unwrap();

        assert_eq!(
            hash,
            "1c4aaba26f36474e9f292099a0ea09663bddbad4e6f3be8f8709bc05ea89c261"
        );
    }

    #[test]
    fn source_map_file_names_are_project_relative() {
        let project_root = Path::new("/workspace/jetton");

        assert_eq!(
            normalize_source_map_file_name(
                "/workspace/jetton/contracts/src/JettonWallet.tolk",
                project_root,
            )
            .unwrap(),
            "contracts/src/JettonWallet.tolk"
        );
        assert_eq!(
            normalize_source_map_file_name("gen/JettonWallet.code.tolk", project_root).unwrap(),
            "gen/JettonWallet.code.tolk"
        );
        assert_eq!(
            normalize_source_map_file_name("@stdlib/common.tolk", project_root).unwrap(),
            "@stdlib/common.tolk"
        );
        assert!(normalize_source_map_file_name("/tmp/outside.tolk", project_root).is_err());
    }
}
