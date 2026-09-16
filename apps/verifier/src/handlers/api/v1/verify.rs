use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Component, Path},
    time::Instant,
};

use axum::{
    Json,
    body::Bytes,
    extract::{Multipart as MultipartExtractor, State, multipart::Multipart},
    http::HeaderMap,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use utoipa::ToSchema;

use crate::{
    blockchain::{is_valid_hash, normalize_code_hash, normalize_hash},
    compilers::{
        CompileGeneratedSource, CompileOutput, CompileRequest, CompileSource, CompilerError,
    },
    error::ApiError,
    payment::PaymentAttemptOutcome,
    registry::VerifiedBundleRequest,
    source_bundle::{
        SourceBundleCompiler, SourceBundleFile, SourceBundleInput, SourceBundleSource,
        compute_source_bundle_hash,
    },
    source_storage::{
        CompilerMetadata, SourceStorageFile, StoreSourceBundleRequest, StoredSourceBundle,
    },
    state::AppState,
    verification::{ResolvedVerificationTarget, VerificationTarget},
};

mod languages;
mod upload_limits;

use super::validation;

const API_KEY_HEADER: &str = "x-verifier-key";
const ALLOWED_SOURCE_PATH_PUNCTUATION: [u8; 6] = *b"/._-@+";
const MAX_SOURCE_DIRECTORY_DEPTH: usize = 16;
const MAX_SOURCE_PATH_CHARS: usize = 128;

#[utoipa::path(
    post,
    path = "/api/v1/verify",
    operation_id = "verify",
    request_body(
        content = VerifyMultipartRequest,
        content_type = "multipart/form-data",
        description = "Multipart verification request. The sources and compile_params parts contain JSON encoded as text."
    ),
    responses(
        (status = 200, description = "Verification completed", body = VerifyResponse),
        (status = 400, description = "Invalid verification request or compilation failure", body = crate::error::ErrorResponse),
        (status = 401, description = "A valid API key is required to set verified_at or skip payment", body = crate::error::ErrorResponse),
        (status = 402, description = "Payment is missing or invalid", body = crate::error::ErrorResponse),
        (status = 404, description = "Current code hash was not found for the requested address", body = crate::error::ErrorResponse),
        (status = 409, description = "Payment is already used or in progress, or the address exists on both TON networks", body = crate::error::ErrorResponse),
        (status = 413, description = "The request exceeds the configured upload limit", body = crate::error::ErrorResponse),
        (status = 502, description = "Compiler, blockchain, payment provider, or source storage failure", body = crate::error::ErrorResponse),
        (status = 503, description = "Verifier is read-only or payment history recovery is in progress", body = crate::error::ErrorResponse)
    ),
    params(
        ("X-Verifier-Key" = Option<String>, Header, description = "API key used to authorize verified_at and verification without payment")
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
    let mut tx_hash = None;
    let mut files = Vec::new();
    let mut seen_fields = BTreeSet::new();

    while let Some(field) = multipart.next_field().await.map_err(ApiError::from)? {
        if let Some(name) = field.name()
            && name != "files"
            && !seen_fields.insert(name.to_owned())
        {
            return Err(ApiError::bad_request(format!(
                "duplicate multipart field: {name}"
            )));
        }
        match field.name() {
            Some("address") => {
                address = Some(field.text().await.map_err(ApiError::from)?);
            }
            Some("code_hash") => {
                code_hash = Some(field.text().await.map_err(ApiError::from)?);
            }
            Some("language") => {
                language = Some(field.text().await.map_err(ApiError::from)?);
            }
            Some("compile_params") => {
                let raw_params = field.text().await.map_err(ApiError::from)?;
                compile_params = serde_json::from_str(&raw_params).map_err(|err| {
                    ApiError::bad_request(format!("invalid compile_params JSON: {err}"))
                })?;
            }
            Some("sources") => {
                let raw_sources = field.text().await.map_err(ApiError::from)?;
                sources = Some(
                    serde_json::from_str::<Vec<SourceMetadata>>(&raw_sources).map_err(|err| {
                        ApiError::bad_request(format!("invalid sources JSON: {err}"))
                    })?,
                );
            }
            Some("verified_at") => {
                let value = field.text().await.map_err(ApiError::from)?;
                let verified_at_millis = value
                    .parse::<u64>()
                    .map_err(|err| ApiError::bad_request(format!("invalid verified_at: {err}")))?;
                verified_at = Some(verified_at_millis / 1_000);
            }
            Some("tx_hash") => {
                tx_hash = Some(field.text().await.map_err(ApiError::from)?);
            }
            Some("files") => {
                upload_limits::ensure_file_slot(files.len())?;
                files.push(upload_limits::read_file_part(field).await?);
            }
            _ => {}
        }
    }

    let has_valid_api_key = state.api_key_matches(
        headers
            .get(API_KEY_HEADER)
            .and_then(|value| value.to_str().ok()),
    );
    if verified_at.is_some() && !has_valid_api_key {
        return Err(ApiError::unauthorized(
            "a valid API key is required to set verified_at".to_owned(),
        ));
    }

    let address = validation::optional_address(address)?;
    let code_hash = validation::optional_code_hash(code_hash)?;

    let language = language
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| ApiError::bad_request("missing required field: language".to_owned()))?;
    if files.is_empty() {
        return Err(ApiError::bad_request(
            "missing required field: files".to_owned(),
        ));
    }

    let target = VerificationTarget { address, code_hash };

    let resolved_target = state.verification_service().resolve_target(target).await?;
    let verified_bundle = find_verified_bundle(state, &resolved_target.code_hash).await?;

    let has_submitted_payment = !has_valid_api_key
        && tx_hash
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty());

    if !has_submitted_payment && let Some(bundle) = &verified_bundle {
        return Ok(already_verified_response(resolved_target.code_hash, bundle));
    }

    if verified_bundle.is_none() && state.read_only() {
        return Err(ApiError::read_only());
    }

    let payment_claim = if has_valid_api_key {
        None
    } else {
        let tx_hash = non_empty_text(tx_hash).ok_or_else(|| {
            ApiError::payment_required("missing required field: tx_hash".to_owned())
        })?;
        let tx_hash = normalize_payment_transaction_hash(&tx_hash)?;
        Some(
            state
                .payment_verifier()
                .claim(&tx_hash, &resolved_target.code_hash)
                .await?,
        )
    };
    let payment_tx_hash = payment_claim
        .as_ref()
        .map(|claim| claim.transaction_hash.clone());

    let task_state = state.clone();
    let task = state.spawn_background_task(async move {
        let started = Instant::now();
        let target_hash = resolved_target.code_hash.clone();
        let result = verify_target(
            &task_state,
            resolved_target,
            verified_bundle,
            language,
            compile_params,
            sources,
            files,
            verified_at,
            payment_tx_hash,
        )
        .await;

        let result = if let Some(claim) = payment_claim {
            let outcome = if result.as_ref().is_err_and(ApiError::is_payment_retryable) {
                PaymentAttemptOutcome::Retryable
            } else {
                PaymentAttemptOutcome::Consumed
            };
            match task_state.payment_verifier().finish(&claim, outcome) {
                Ok(()) => result,
                Err(error) => Err(ApiError::from(error)),
            }
        } else {
            result
        };

        tracing::info!(
            operation = "verify",
            target = %target_hash,
            duration_ms = started.elapsed().as_millis(),
            outcome = if result.is_ok() { "completed" } else { "failed" },
            "verification request finished"
        );

        result
    });

    task.await
        .map_err(|error| ApiError::internal(format!("verification task failed: {error}")))?
}

#[allow(clippy::too_many_arguments)]
async fn verify_target(
    state: &AppState,
    resolved_target: ResolvedVerificationTarget,
    verified_bundle: Option<StoredSourceBundle>,
    language: String,
    compile_params: Value,
    sources: Option<Vec<SourceMetadata>>,
    files: Vec<ReceivedFile>,
    verified_at: Option<u64>,
    payment_tx_hash: Option<String>,
) -> Result<Json<VerifyResponse>, ApiError> {
    let verified_bundle = match verified_bundle {
        Some(bundle) => Some(bundle),
        None => find_verified_bundle(state, &resolved_target.code_hash).await?,
    };
    if let Some(bundle) = verified_bundle {
        return Ok(already_verified_response(
            resolved_target.code_hash,
            &bundle,
        ));
    }

    tracing::info!(
        operation = "verify",
        target = %resolved_target.code_hash,
        outcome = "started",
        "verification started"
    );

    let CompileInput {
        configuration,
        sources: mut retained_sources,
    } = prepare_compile_input(&language, &compile_params, sources, files)?;
    let compiled = run_compiler(
        state,
        &resolved_target.code_hash,
        &configuration,
        &compile_params,
        retained_sources.clone(),
    )
    .await?;
    let compiled_code_hash = normalize_code_hash(&compiled.code_hash);
    let mut verification_result =
        VerificationResult::from_hashes(&resolved_target.code_hash, &compiled_code_hash);
    let (source_bundle_hash, storage_revision) = match verification_result {
        VerificationResult::Match => {
            if let Some(used_source_paths) = compiled.used_source_paths.clone() {
                retained_sources = select_used_sources(&retained_sources, &used_source_paths)?;
            }

            let mut storage_files = storage_files_from_sources(&retained_sources);
            merge_generated_sources(&mut storage_files, compiled.generated_sources)?;
            let source_bundle_hash = compute_source_bundle_hash(SourceBundleInput {
                compiler: SourceBundleCompiler {
                    language: &configuration.language,
                    version: &configuration.compiler_version,
                    entrypoint: &configuration.entrypoint,
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
                    payment_tx_hash,
                    verified_at,
                    compiler: CompilerMetadata {
                        language: configuration.language.clone(),
                        version: configuration.compiler_version.clone(),
                        entrypoint: configuration.entrypoint.clone(),
                        params: compile_params.clone(),
                    },
                    files: storage_files,
                    source_map: compiled.source_map,
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

    tracing::info!(
        operation = "verify",
        target = %resolved_target.code_hash,
        language = %configuration.language,
        compiled_code_hash = %compiled_code_hash,
        source_bundle_hash,
        outcome = %verification_result,
        "verification result"
    );

    Ok(Json(VerifyResponse {
        code_hash: resolved_target.code_hash,
        compiled_code_hash: Some(compiled_code_hash),
        verification_result,
        source_bundle_hash,
        storage_revision,
    }))
}

async fn run_compiler(
    state: &AppState,
    code_hash: &str,
    configuration: &CompileConfiguration,
    compile_params: &Value,
    sources: Vec<CompileSource>,
) -> Result<CompileOutput, CompilerError> {
    state
        .compile(
            code_hash,
            CompileRequest {
                language: configuration.language.clone(),
                compiler_version: configuration.compiler_version.clone(),
                entrypoint: configuration.entrypoint.clone(),
                import_mappings: configuration.import_mappings.clone(),
                compile_params: compile_params.clone(),
                sources,
            },
        )
        .await
}

fn select_used_sources(
    sources: &[CompileSource],
    used_source_paths: &[String],
) -> Result<Vec<CompileSource>, CompilerError> {
    if used_source_paths.is_empty() {
        return Err(CompilerError::InvalidOutput(
            "used_source_paths must contain at least one source".to_owned(),
        ));
    }
    if used_source_paths
        .windows(2)
        .any(|paths| paths[0] >= paths[1])
    {
        return Err(CompilerError::InvalidOutput(
            "used_source_paths must be sorted and duplicate-free".to_owned(),
        ));
    }

    let available_paths = sources
        .iter()
        .map(|source| source.path.as_str())
        .collect::<BTreeSet<_>>();
    for path in used_source_paths {
        if !available_paths.contains(path.as_str()) {
            return Err(CompilerError::InvalidOutput(format!(
                "used_source_paths contains a path absent from the compiler request: {path}"
            )));
        }
    }

    let used_paths = used_source_paths
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let selected = sources
        .iter()
        .filter(|source| used_paths.contains(source.path.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    if selected.len() != used_source_paths.len() {
        return Err(CompilerError::InvalidOutput(
            "used_source_paths could not be mapped to compiler sources".to_owned(),
        ));
    }

    Ok(selected)
}

fn storage_files_from_sources(sources: &[CompileSource]) -> Vec<SourceStorageFile> {
    sources
        .iter()
        .map(|source| SourceStorageFile {
            path: source.path.clone(),
            content: source.content.clone(),
            include_in_command: source.include_in_command,
            is_stdlib: source.is_stdlib,
            has_include_directives: source.has_include_directives,
        })
        .collect()
}

fn non_empty_text(value: Option<String>) -> Option<String> {
    value.filter(|value| !value.trim().is_empty())
}

fn normalize_payment_transaction_hash(value: &str) -> Result<String, ApiError> {
    let transaction_hash = normalize_hash(value);
    if !is_valid_hash(&transaction_hash) {
        return Err(ApiError::bad_request(
            "payment_tx_hash_invalid: transaction hash must be 64 hexadecimal characters or a 32-byte base64 value"
                .to_owned(),
        ));
    }
    Ok(transaction_hash)
}

fn already_verified_response(
    code_hash: String,
    bundle: &StoredSourceBundle,
) -> Json<VerifyResponse> {
    Json(VerifyResponse {
        code_hash,
        compiled_code_hash: None,
        verification_result: VerificationResult::AlreadyVerified,
        source_bundle_hash: Some(bundle.manifest.source_bundle_hash.clone()),
        storage_revision: Some(bundle.storage_revision.clone()),
    })
}

async fn find_verified_bundle(
    state: &AppState,
    code_hash: &str,
) -> Result<Option<StoredSourceBundle>, ApiError> {
    Ok(state
        .verification_registry()
        .verified_bundle(VerifiedBundleRequest {
            code_hash: code_hash.to_owned(),
        })
        .await?
        .bundle)
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

    Ok(CompileInput {
        configuration: CompileConfiguration {
            language,
            compiler_version,
            import_mappings,
            entrypoint,
        },
        sources: compile_sources,
    })
}

fn validate_source_path(path: &str) -> Result<(), ApiError> {
    if path.chars().count() > MAX_SOURCE_PATH_CHARS {
        return Err(ApiError::bad_request(format!(
            "source path must be no longer than {MAX_SOURCE_PATH_CHARS} characters"
        )));
    }

    validate_relative_path("source path", path)?;
    if path.bytes().filter(|character| *character == b'/').count() > MAX_SOURCE_DIRECTORY_DEPTH {
        return Err(ApiError::bad_request(format!(
            "source path must contain no more than {MAX_SOURCE_DIRECTORY_DEPTH} directories"
        )));
    }
    validate_source_path_components(path)?;
    validate_source_extension_count(path)
}

fn validate_source_path_components(path: &str) -> Result<(), ApiError> {
    if !path.bytes().all(|character| {
        character.is_ascii_alphanumeric() || ALLOWED_SOURCE_PATH_PUNCTUATION.contains(&character)
    }) {
        return Err(ApiError::bad_request(
            "source path components may contain only ASCII letters, numbers, '.', '_', '-', '@' and '+'"
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
    }

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

struct CompileInput {
    configuration: CompileConfiguration,
    sources: Vec<CompileSource>,
}

struct CompileConfiguration {
    language: String,
    compiler_version: String,
    import_mappings: BTreeMap<String, String>,
    entrypoint: String,
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
    /// Finalized TON transaction hash for this verification attempt.
    tx_hash: Option<String>,
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
    use super::{select_used_sources, validate_relative_path, validate_source_path};
    use crate::compilers::{CompileSource, CompilerError};

    fn compile_source(path: &str) -> CompileSource {
        CompileSource {
            path: path.to_owned(),
            content: path.to_owned(),
            is_entrypoint: path == "main.tolk",
            include_in_command: None,
            is_stdlib: None,
            has_include_directives: None,
        }
    }

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

    #[test]
    fn used_source_selection_preserves_only_reported_sources() {
        let sources = [
            compile_source("z.tolk"),
            compile_source("main.tolk"),
            compile_source("unused.tolk"),
        ];
        let selected =
            select_used_sources(&sources, &["main.tolk".to_owned(), "z.tolk".to_owned()])
                .expect("valid worker paths should be selected");

        assert_eq!(selected.len(), 2);
        assert_eq!(selected[0].path, "z.tolk");
        assert_eq!(selected[1].path, "main.tolk");
    }

    #[test]
    fn used_source_selection_rejects_malformed_worker_output() {
        let sources = [compile_source("main.tolk"), compile_source("unused.tolk")];
        for paths in [
            Vec::new(),
            vec!["unused.tolk".to_owned(), "main.tolk".to_owned()],
            vec!["main.tolk".to_owned(), "main.tolk".to_owned()],
            vec!["missing.tolk".to_owned()],
        ] {
            assert!(matches!(
                select_used_sources(&sources, &paths),
                Err(CompilerError::InvalidOutput(_))
            ));
        }
    }
}
