use std::{
    collections::BTreeMap,
    path::{Component, Path},
};

use axum::{
    Json,
    body::Bytes,
    extract::{
        Multipart as MultipartExtractor, State,
        multipart::{Field, Multipart},
    },
    http::HeaderMap,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use utoipa::ToSchema;

use crate::{
    blockchain::normalize_code_hash,
    compilers::{CompileGeneratedSource, CompileRequest, CompileSource},
    error::ApiError,
    registry::VerifiedBundleRequest,
    source_bundle::{
        SourceBundleCompiler, SourceBundleFile, SourceBundleInput, SourceBundleSource,
        compute_source_bundle_hash,
    },
    source_storage::{CompilerMetadata, SourceStorageFile, StoreSourceBundleRequest},
    state::AppState,
    verification::{ResolvedVerificationTarget, VerificationTarget},
};

mod languages;

const API_KEY_HEADER: &str = "x-verifier-key";
const MAX_SOURCE_PATH_CHARS: usize = 128;

#[utoipa::path(
    post,
    path = "/api/v1/verify",
    request_body(
        content = VerifyMultipartRequest,
        content_type = "multipart/form-data",
        description = "Multipart verification request. The sources and compile_params parts contain JSON encoded as text."
    ),
    responses(
        (status = 200, description = "Verification completed", body = VerifyResponse),
        (status = 400, description = "Invalid verification request or compilation mismatch input", body = crate::error::ErrorResponse),
        (status = 401, description = "A valid API key is required to set verified_at", body = crate::error::ErrorResponse),
        (status = 404, description = "Current code hash was not found for the requested address", body = crate::error::ErrorResponse),
        (status = 502, description = "Compiler, blockchain, or source storage failure", body = crate::error::ErrorResponse)
    ),
    params(
        ("X-Verifier-Key" = Option<String>, Header, description = "API key required only when verified_at is provided")
    ),
    tag = "verification"
)]
pub async fn handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    multipart: MultipartExtractor,
) -> Result<impl IntoResponse, ApiError> {
    handle_multipart(&state, &headers, multipart).await
}

async fn handle_multipart(
    state: &AppState,
    headers: &HeaderMap,
    mut multipart: Multipart,
) -> Result<Json<VerifyResponse>, ApiError> {
    let mut address = None;
    let mut code_hash = None;
    let mut language = None;
    let mut compile_params = json!({});
    let mut sources = None;
    let mut verified_at = None;
    let mut files = Vec::new();

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|err| ApiError::bad_request(err.to_string()))?
    {
        match field.name() {
            Some("address") => {
                address = Some(
                    field
                        .text()
                        .await
                        .map_err(|err| ApiError::bad_request(err.to_string()))?,
                );
            }
            Some("code_hash") => {
                code_hash = Some(
                    field
                        .text()
                        .await
                        .map_err(|err| ApiError::bad_request(err.to_string()))?,
                );
            }
            Some("language") => {
                language = Some(
                    field
                        .text()
                        .await
                        .map_err(|err| ApiError::bad_request(err.to_string()))?,
                );
            }
            Some("compile_params") => {
                let raw_params = field
                    .text()
                    .await
                    .map_err(|err| ApiError::bad_request(err.to_string()))?;
                compile_params = serde_json::from_str(&raw_params).map_err(|err| {
                    ApiError::bad_request(format!("invalid compile_params JSON: {err}"))
                })?;
            }
            Some("sources") => {
                let raw_sources = field
                    .text()
                    .await
                    .map_err(|err| ApiError::bad_request(err.to_string()))?;
                sources = Some(
                    serde_json::from_str::<Vec<SourceMetadata>>(&raw_sources).map_err(|err| {
                        ApiError::bad_request(format!("invalid sources JSON: {err}"))
                    })?,
                );
            }
            Some("verified_at") => {
                let value = field
                    .text()
                    .await
                    .map_err(|err| ApiError::bad_request(err.to_string()))?;
                let verified_at_millis = value
                    .parse::<u64>()
                    .map_err(|err| ApiError::bad_request(format!("invalid verified_at: {err}")))?;
                verified_at = Some(verified_at_millis / 1_000);
            }
            Some("files") => {
                files.push(read_file_part(field).await?);
            }
            _ => {}
        }
    }

    if verified_at.is_some()
        && !state.api_key_matches(
            headers
                .get(API_KEY_HEADER)
                .and_then(|value| value.to_str().ok()),
        )
    {
        return Err(ApiError::unauthorized(
            "a valid API key is required to set verified_at".to_owned(),
        ));
    }

    let target = VerificationTarget {
        address: non_empty_text(address),
        code_hash: non_empty_text(code_hash),
    };

    let language = language
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| ApiError::bad_request("missing required field: language".to_owned()))?;

    if files.is_empty() {
        return Err(ApiError::bad_request(
            "missing required field: files".to_owned(),
        ));
    }

    let compile_input = prepare_compile_input(&language, &compile_params, sources, files)?;
    let resolved_target = state.verification_service().resolve_target(target).await?;
    if let Some(bundle) = state
        .verification_registry()
        .verified_bundle(VerifiedBundleRequest {
            code_hash: resolved_target.code_hash.clone(),
        })
        .await?
        .bundle
    {
        return Ok(Json(VerifyResponse {
            code_hash: resolved_target.code_hash,
            compiled_code_hash: None,
            verification_result: VerificationResult::AlreadyVerified,
            source_bundle_hash: Some(bundle.manifest.source_bundle_hash),
            storage_revision: Some(bundle.storage_revision),
        }));
    }
    let compiled = state
        .compiler_service()
        .compile(CompileRequest {
            language: compile_input.language.clone(),
            compiler_version: compile_input.compiler_version.clone(),
            entrypoint: compile_input.entrypoint.clone(),
            import_mappings: compile_input.import_mappings.clone(),
            compile_params: compile_params.clone(),
            sources: compile_input.compile_sources,
        })
        .await?;
    let compiled_code_hash = normalize_code_hash(&compiled.code_hash);
    let compiled_source_map_data = compiled.source_map.clone();
    let mut verification_result =
        VerificationResult::from_hashes(&resolved_target.code_hash, &compiled_code_hash);
    let (source_bundle_hash, storage_revision) = match verification_result {
        VerificationResult::Match => {
            let mut stored_sources = compile_input.sources.clone();
            let mut storage_files = compile_input.storage_files.clone();
            merge_generated_sources(
                &mut stored_sources,
                &mut storage_files,
                compiled.generated_sources,
            )?;
            let source_bundle_hash = compute_source_bundle_hash(SourceBundleInput {
                compiler: SourceBundleCompiler {
                    language: &compile_input.language,
                    version: &compile_input.compiler_version,
                    entrypoint: &compile_input.entrypoint,
                    params: &compile_params,
                },
                sources: storage_files
                    .iter()
                    .map(SourceBundleSource::from_storage_file)
                    .collect(),
                files: storage_files
                    .iter()
                    .map(|file| SourceBundleFile {
                        path: &file.path,
                        bytes: file.content.as_bytes(),
                    })
                    .collect(),
            })?;
            let stored = state
                .verification_registry()
                .store_verified_bundle(StoreSourceBundleRequest {
                    code_hash: resolved_target.code_hash.clone(),
                    source_bundle_hash: source_bundle_hash.clone(),
                    verified_at,
                    compiler: CompilerMetadata {
                        language: compile_input.language.clone(),
                        version: compile_input.compiler_version.clone(),
                        entrypoint: compile_input.entrypoint.clone(),
                        params: compile_params.clone(),
                    },
                    files: storage_files,
                    source_map: compiled_source_map_data,
                })
                .await?;
            if !stored.storage.created {
                verification_result = VerificationResult::AlreadyVerified;
            }
            (
                Some(stored.bundle.manifest.source_bundle_hash),
                Some(stored.storage.revision),
            )
        }
        VerificationResult::Mismatch => (None, None),
        VerificationResult::AlreadyVerified => {
            unreachable!("hash comparison cannot produce an already-verified result")
        }
    };

    print_verify_request(
        &resolved_target,
        &compile_input.language,
        &compile_params,
        &compile_input.sources,
        &compiled_code_hash,
        source_bundle_hash.as_deref(),
        verification_result,
    );

    Ok(Json(VerifyResponse {
        code_hash: resolved_target.code_hash,
        compiled_code_hash: Some(compiled_code_hash),
        verification_result,
        source_bundle_hash,
        storage_revision,
    }))
}

fn non_empty_text(value: Option<String>) -> Option<String> {
    value.filter(|value| !value.trim().is_empty())
}

async fn read_file_part(field: Field<'_>) -> Result<ReceivedFile, ApiError> {
    let file_name = field.file_name().map(ToOwned::to_owned);
    let content = field
        .bytes()
        .await
        .map_err(|err| ApiError::bad_request(err.to_string()))?;

    Ok(ReceivedFile { file_name, content })
}

fn prepare_compile_input(
    language: &str,
    compile_params: &Value,
    sources: Option<Vec<SourceMetadata>>,
    files: Vec<ReceivedFile>,
) -> Result<CompileInput, ApiError> {
    let sources = sources
        .ok_or_else(|| ApiError::bad_request("missing required field: sources".to_owned()))?;
    let files = match_files_to_sources(&sources, files)?;
    let language_input = languages::prepare(language, compile_params, &sources, &files)?;
    let language = language_input.language;
    let entrypoint = language_input.entrypoint;
    let compiler_version = language_input.compiler_version;
    let import_mappings = language_input.import_mappings;
    validate_import_mappings(&import_mappings)?;
    let compile_sources = build_compile_sources(&sources, files)?;
    let storage_files = compile_sources
        .iter()
        .map(|source| SourceStorageFile {
            path: source.path.clone(),
            content: source.content.clone(),
            include_in_command: source.include_in_command,
            is_stdlib: source.is_stdlib,
            has_include_directives: source.has_include_directives,
        })
        .collect();

    Ok(CompileInput {
        language,
        compiler_version,
        import_mappings,
        entrypoint,
        compile_sources,
        sources,
        storage_files,
    })
}

fn validate_source_path(path: &str) -> Result<(), ApiError> {
    if path.chars().count() > MAX_SOURCE_PATH_CHARS {
        return Err(ApiError::bad_request(format!(
            "source path must be no longer than {MAX_SOURCE_PATH_CHARS} characters"
        )));
    }

    validate_relative_path("source path", path)?;
    validate_source_path_components(path)?;
    validate_source_extension_count(path)
}

fn validate_source_path_components(path: &str) -> Result<(), ApiError> {
    if !path.bytes().all(|character| {
        character.is_ascii_alphanumeric() || matches!(character, b'/' | b'.' | b'_' | b'-')
    }) {
        return Err(ApiError::bad_request(
            "source path components may contain only ASCII letters, numbers, '.', '_' and '-'"
                .to_owned(),
        ));
    }
    if path.split('/').any(|component| component.ends_with('.')) {
        return Err(ApiError::bad_request(
            "source path component must not end with '.'".to_owned(),
        ));
    }

    Ok(())
}

fn validate_source_extension_count(path: &str) -> Result<(), ApiError> {
    let file_name = path
        .rsplit_once('/')
        .map_or(path, |(_, file_name)| file_name);
    let source_extension_count = file_name
        .split('.')
        .skip(1)
        .filter(|extension| languages::is_known_source_extension(extension))
        .take(2)
        .count();
    if source_extension_count > 1 {
        return Err(ApiError::bad_request(
            "source path must not contain multiple source extensions".to_owned(),
        ));
    }

    Ok(())
}

fn validate_import_mappings(import_mappings: &BTreeMap<String, String>) -> Result<(), ApiError> {
    for (prefix, target) in import_mappings {
        validate_relative_path("import mapping prefix", prefix)?;
        validate_relative_path("import mapping target", target)?;
    }

    Ok(())
}

fn validate_relative_path(name: &str, value: &str) -> Result<(), ApiError> {
    if value.trim().is_empty() {
        return Err(ApiError::bad_request(format!("{name} is empty")));
    }
    if value.trim() != value {
        return Err(ApiError::bad_request(format!(
            "{name} has leading or trailing whitespace"
        )));
    }
    if value.chars().any(char::is_control) {
        return Err(ApiError::bad_request(format!(
            "{name} contains a control character"
        )));
    }
    if value.contains('\\') {
        return Err(ApiError::bad_request(format!(
            "{name} must use '/' separators"
        )));
    }
    if value.starts_with('~') {
        return Err(ApiError::bad_request(format!(
            "{name} must not start with '~'"
        )));
    }
    if matches!(
        value.as_bytes(),
        [drive, b':', ..] if drive.is_ascii_alphabetic()
    ) {
        return Err(ApiError::bad_request(format!(
            "{name} must not use a Windows drive prefix"
        )));
    }

    let path = Path::new(value);
    if path.is_absolute() {
        return Err(ApiError::bad_request(format!("{name} must be relative")));
    }

    for component in value.split('/') {
        if component.is_empty() {
            return Err(ApiError::bad_request(format!(
                "{name} contains an empty component"
            )));
        }
        if component == "." {
            return Err(ApiError::bad_request(format!(
                "{name} contains an invalid component"
            )));
        }
        if component.eq_ignore_ascii_case(".git") {
            return Err(ApiError::bad_request(format!(
                "{name} contains reserved '.git' component"
            )));
        }
    }

    for component in path.components() {
        match component {
            Component::Normal(_) => {}
            Component::CurDir
            | Component::ParentDir
            | Component::RootDir
            | Component::Prefix(_) => {
                return Err(ApiError::bad_request(format!(
                    "{name} contains an invalid component"
                )));
            }
        }
    }

    Ok(())
}

fn match_files_to_sources(
    sources: &[SourceMetadata],
    files: Vec<ReceivedFile>,
) -> Result<BTreeMap<String, ReceivedFile>, ApiError> {
    let mut files_by_path = BTreeMap::new();
    for file in files {
        let file_name = file
            .file_name
            .clone()
            .ok_or_else(|| ApiError::bad_request("file part is missing filename".to_owned()))?;
        validate_source_path(&file_name)?;
        if files_by_path.insert(file_name.clone(), file).is_some() {
            return Err(ApiError::bad_request(format!(
                "duplicate uploaded file path: {file_name}"
            )));
        }
    }

    for source in sources {
        if !files_by_path.contains_key(&source.path) {
            return Err(ApiError::bad_request(format!(
                "source metadata has no uploaded file: {}",
                source.path
            )));
        }
    }

    for file_path in files_by_path.keys() {
        if !sources.iter().any(|source| source.path == *file_path) {
            return Err(ApiError::bad_request(format!(
                "uploaded file has no source metadata: {file_path}"
            )));
        }
    }

    Ok(files_by_path)
}

fn build_compile_sources(
    sources: &[SourceMetadata],
    mut files: BTreeMap<String, ReceivedFile>,
) -> Result<Vec<CompileSource>, ApiError> {
    let mut compile_sources = Vec::with_capacity(sources.len());
    for source in sources {
        let file = files.remove(&source.path).ok_or_else(|| {
            ApiError::bad_request(format!(
                "source metadata has no uploaded file: {}",
                source.path
            ))
        })?;
        let content = String::from_utf8(file.content.to_vec()).map_err(|err| {
            ApiError::bad_request(format!("source is not valid UTF-8: {}: {err}", source.path))
        })?;
        compile_sources.push(CompileSource {
            path: source.path.clone(),
            content,
            is_entrypoint: source.is_entrypoint,
            include_in_command: source.include_in_command,
            is_stdlib: source.is_stdlib,
            has_include_directives: source.has_include_directives,
        });
    }

    Ok(compile_sources)
}

fn merge_generated_sources(
    sources: &mut Vec<SourceMetadata>,
    files: &mut Vec<SourceStorageFile>,
    generated_sources: Vec<CompileGeneratedSource>,
) -> Result<(), ApiError> {
    for generated in generated_sources {
        validate_source_path(&generated.path)?;
        let content = generated.content;
        match files.iter().find(|file| file.path == generated.path) {
            Some(existing) if existing.content == content => {}
            Some(_) => {
                return Err(ApiError::bad_request(format!(
                    "generated source conflicts with uploaded file: {}",
                    generated.path
                )));
            }
            None => files.push(SourceStorageFile {
                path: generated.path.clone(),
                content,
                include_in_command: None,
                is_stdlib: None,
                has_include_directives: None,
            }),
        }

        if !sources.iter().any(|source| source.path == generated.path) {
            sources.push(SourceMetadata {
                path: generated.path,
                is_entrypoint: false,
                include_in_command: None,
                is_stdlib: None,
                has_include_directives: None,
            });
        }
    }

    sources.sort_by(|left, right| left.path.cmp(&right.path));
    files.sort_by(|left, right| left.path.cmp(&right.path));

    Ok(())
}

impl<'a> SourceBundleSource<'a> {
    fn from_storage_file(file: &'a SourceStorageFile) -> Self {
        Self {
            path: &file.path,
            include_in_command: file.include_in_command,
            is_stdlib: file.is_stdlib,
            has_include_directives: file.has_include_directives,
        }
    }
}

fn print_verify_request(
    target: &ResolvedVerificationTarget,
    language: &str,
    compile_params: &Value,
    sources: &[SourceMetadata],
    compiled_code_hash: &str,
    source_bundle_hash: Option<&str>,
    verification_result: VerificationResult,
) {
    println!("verification request");
    println!("address: {}", target.address.as_deref().unwrap_or("<none>"));
    println!("code_hash: {}", target.code_hash);
    println!("compiled_code_hash: {compiled_code_hash}");
    println!(
        "source_bundle_hash: {}",
        source_bundle_hash.unwrap_or("<none>")
    );
    println!("verification_result: {verification_result}");
    println!("language: {language}");
    println!("compile_params: {compile_params}");

    for source in sources {
        println!(
            "source: path={} is_entrypoint={}",
            source.path, source.is_entrypoint
        );
    }
}

struct CompileInput {
    language: String,
    compiler_version: String,
    import_mappings: BTreeMap<String, String>,
    entrypoint: String,
    compile_sources: Vec<CompileSource>,
    sources: Vec<SourceMetadata>,
    storage_files: Vec<SourceStorageFile>,
}

#[derive(Debug)]
struct ReceivedFile {
    file_name: Option<String>,
    content: Bytes,
}

#[derive(Clone, Debug, Deserialize, ToSchema)]
pub(super) struct SourceMetadata {
    path: String,
    is_entrypoint: bool,
    #[serde(default)]
    include_in_command: Option<bool>,
    #[serde(default)]
    is_stdlib: Option<bool>,
    #[serde(default)]
    has_include_directives: Option<bool>,
}

#[derive(Debug, ToSchema)]
#[allow(dead_code)]
pub(super) struct VerifyMultipartRequest {
    #[schema(nullable = false, example = "EQD...")]
    address: Option<String>,
    #[schema(
        nullable = false,
        example = "a873d8c2d163f7fa10bbe38769706f0554505e8ea2dcea3f115288db8becf2ab"
    )]
    code_hash: Option<String>,
    #[schema(example = "tolk")]
    language: String,
    #[schema(
        content_media_type = "application/json",
        example = r#"{"compiler_version":"1.4.1"}"#
    )]
    compile_params: String,
    #[schema(
        content_media_type = "application/json",
        example = r#"[{"path":"main.tolk","is_entrypoint":true}]"#
    )]
    sources: String,
    // TODO: Remove this field after migrating contracts from the legacy verifier.
    /// Original verification Unix timestamp in milliseconds.
    /// Requires a valid `X-Verifier-Key` header.
    #[schema(nullable = false, example = 1_700_000_000_000_u64)]
    verified_at: Option<u64>,
    #[schema(
        value_type = String,
        format = Binary,
        content_media_type = "application/octet-stream"
    )]
    files: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub(super) struct VerifyResponse {
    code_hash: String,
    compiled_code_hash: Option<String>,
    verification_result: VerificationResult,
    source_bundle_hash: Option<String>,
    storage_revision: Option<String>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub(super) enum VerificationResult {
    AlreadyVerified,
    Match,
    Mismatch,
}

impl VerificationResult {
    fn from_hashes(target: &str, compiled: &str) -> Self {
        if target == compiled {
            Self::Match
        } else {
            Self::Mismatch
        }
    }
}

impl std::fmt::Display for VerificationResult {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AlreadyVerified => formatter.write_str("already_verified"),
            Self::Match => formatter.write_str("match"),
            Self::Mismatch => formatter.write_str("mismatch"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{validate_relative_path, validate_source_path};

    #[test]
    fn source_path_rejects_control_characters() {
        for path in ["main\0.tolk", "main\n.tolk", "main\r.tolk", "main\t.tolk"] {
            assert!(
                validate_source_path(path).is_err(),
                "path should be rejected: {path:?}"
            );
        }
    }

    #[test]
    fn import_mapping_rejects_unsafe_paths() {
        for path in [
            "../contracts",
            "~/contracts",
            "C:/contracts",
            "contracts/./imports",
            "contracts//imports",
            ".git/imports",
            "contracts\n",
        ] {
            assert!(
                validate_relative_path("import mapping target", path).is_err(),
                "import mapping path should be rejected: {path:?}"
            );
        }
    }
}
