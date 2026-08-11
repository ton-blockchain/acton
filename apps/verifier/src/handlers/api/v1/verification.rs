use axum::{
    Json,
    extract::{Query, State},
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;

use crate::{
    blockchain::normalize_code_hash,
    error::ApiError,
    registry::{
        AbiContractsRequest, LastVerifiedRequest, VerificationStatisticsHistoryReceipt,
        VerificationStatisticsReceipt, VerificationStatusReceipt, VerificationStatusRequest,
        VerifiedBundleRequest,
    },
    registry_index::{
        IndexedAbiContract, IndexedCompilerVersionStatistics, IndexedLanguageStatistics,
        IndexedVerificationStatisticsHistoryItem, IndexedVerifiedBundleSummary,
    },
    source_storage::{CompilerMetadata, SourceMapData, StoredSourceBundle, StoredSourceFile},
    state::AppState,
    verification::VerificationTarget,
};

const DEFAULT_PAGE_LIMIT: usize = 50;
const MAX_PAGE_LIMIT: usize = 100;

#[utoipa::path(
    get,
    path = "/api/v1/verification/status",
    params(
        ("address" = Option<String>, Query, description = "TON address to resolve to the current code hash"),
        ("code_hash" = Option<String>, Query, description = "Code hash to check directly")
    ),
    responses(
        (status = 200, description = "Verification status for the resolved code hash", body = VerificationStatusResponse),
        (status = 400, description = "Invalid or missing verification target", body = crate::error::ErrorResponse),
        (status = 404, description = "Current code hash was not found for the requested address", body = crate::error::ErrorResponse),
        (status = 502, description = "Blockchain or registry lookup failure", body = crate::error::ErrorResponse)
    ),
    tag = "verification"
)]
pub async fn status_handler(
    State(state): State<AppState>,
    Query(query): Query<VerificationQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let resolved_target = state
        .verification_service()
        .resolve_target(query.into_target())
        .await?;
    let status = state
        .verification_registry()
        .status(VerificationStatusRequest {
            code_hash: resolved_target.code_hash.clone(),
        })
        .await?;

    Ok(Json(VerificationStatusResponse::new(
        resolved_target.code_hash,
        &status,
    )))
}

#[utoipa::path(
    get,
    path = "/api/v1/verification/source",
    params(
        ("address" = Option<String>, Query, description = "TON address to resolve to the current code hash"),
        ("code_hash" = Option<String>, Query, description = "Code hash to load the verified source bundle for")
    ),
    responses(
        (status = 200, description = "Verified source bundle for the resolved code hash", body = VerificationSourceResponse),
        (status = 400, description = "Invalid or missing verification target", body = crate::error::ErrorResponse),
        (status = 404, description = "Current code hash was not found for the requested address", body = crate::error::ErrorResponse),
        (status = 502, description = "Blockchain, registry, or source lookup failure", body = crate::error::ErrorResponse)
    ),
    tag = "verification"
)]
pub async fn source_handler(
    State(state): State<AppState>,
    Query(query): Query<VerificationQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let resolved_target = state
        .verification_service()
        .resolve_target(query.into_target())
        .await?;
    let receipt = state
        .verification_registry()
        .verified_bundle(VerifiedBundleRequest {
            code_hash: resolved_target.code_hash.clone(),
        })
        .await?;
    let bundle = receipt.bundle.map(SourceBundleResponse::from);

    Ok(Json(VerificationSourceResponse {
        code_hash: resolved_target.code_hash,
        verified: bundle.is_some(),
        bundle,
    }))
}

#[utoipa::path(
    get,
    path = "/api/v1/last_verified",
    params(
        ("limit" = Option<usize>, Query, description = "Maximum number of records to return. Defaults to 50 and is capped at 100."),
        ("offset" = Option<usize>, Query, description = "Number of records to skip for pagination.")
    ),
    responses(
        (status = 200, description = "Latest verified source bundles", body = LastVerifiedResponse),
        (status = 502, description = "Registry lookup failure", body = crate::error::ErrorResponse)
    ),
    tag = "verification"
)]
pub async fn last_verified_handler(
    State(state): State<AppState>,
    Query(query): Query<PaginationQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let receipt = state
        .verification_registry()
        .last_verified(LastVerifiedRequest {
            limit: page_limit(query.limit),
            offset: query.offset.unwrap_or(0),
        })
        .await?;

    Ok(Json(LastVerifiedResponse {
        items: receipt
            .items
            .into_iter()
            .map(LastVerifiedItemResponse::from)
            .collect(),
        total: receipt.total,
    }))
}

#[utoipa::path(
    get,
    path = "/api/v1/statistics",
    responses(
        (status = 200, description = "Verified source bundle counts grouped by language and compiler version", body = VerificationStatisticsResponse),
        (status = 502, description = "Registry lookup failure", body = crate::error::ErrorResponse)
    ),
    tag = "verification"
)]
pub async fn statistics_handler(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    let receipt = state.verification_registry().statistics().await?;

    Ok(Json(VerificationStatisticsResponse::from(receipt)))
}

#[utoipa::path(
    get,
    path = "/api/v1/statistics/history",
    responses(
        (status = 200, description = "All verified source bundles with their verification timestamp, compiler, and compiler version", body = VerificationStatisticsHistoryResponse),
        (status = 502, description = "Registry lookup failure", body = crate::error::ErrorResponse)
    ),
    tag = "verification"
)]
pub async fn statistics_history_handler(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    let receipt = state.verification_registry().statistics_history().await?;

    Ok(Json(VerificationStatisticsHistoryResponse::from(receipt)))
}

#[utoipa::path(
    get,
    path = "/api/v1/abi",
    params(
        ("code_hash" = Option<String>, Query, description = "Optional code hash filter."),
        ("limit" = Option<usize>, Query, description = "Maximum number of records to return. Defaults to 50 and is capped at 100."),
        ("offset" = Option<usize>, Query, description = "Number of records to skip for pagination.")
    ),
    responses(
        (status = 200, description = "Tolk ABI records indexed from verified contracts", body = AbiContractsResponse),
        (status = 502, description = "Registry lookup failure", body = crate::error::ErrorResponse)
    ),
    tag = "verification"
)]
pub async fn abi_handler(
    State(state): State<AppState>,
    Query(query): Query<AbiQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let receipt = state
        .verification_registry()
        .abi_contracts(AbiContractsRequest {
            code_hash: non_empty_code_hash(query.code_hash),
            limit: page_limit(query.limit),
            offset: query.offset.unwrap_or(0),
        })
        .await?;

    Ok(Json(AbiContractsResponse {
        items: receipt
            .items
            .into_iter()
            .map(AbiContractResponse::from)
            .collect(),
    }))
}

#[derive(Debug, Deserialize)]
pub(super) struct VerificationQuery {
    address: Option<String>,
    code_hash: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct PaginationQuery {
    limit: Option<usize>,
    offset: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub(super) struct AbiQuery {
    code_hash: Option<String>,
    limit: Option<usize>,
    offset: Option<usize>,
}

impl VerificationQuery {
    fn into_target(self) -> VerificationTarget {
        VerificationTarget {
            address: non_empty_text(self.address),
            code_hash: non_empty_text(self.code_hash),
        }
    }
}

fn non_empty_text(value: Option<String>) -> Option<String> {
    value.filter(|value| !value.trim().is_empty())
}

fn non_empty_code_hash(value: Option<String>) -> Option<String> {
    non_empty_text(value).map(|value| normalize_code_hash(value.trim()))
}

fn page_limit(limit: Option<usize>) -> usize {
    limit.unwrap_or(DEFAULT_PAGE_LIMIT).clamp(1, MAX_PAGE_LIMIT)
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub(super) struct VerificationStatusResponse {
    code_hash: String,
    verified: bool,
}

impl VerificationStatusResponse {
    const fn new(code_hash: String, status: &VerificationStatusReceipt) -> Self {
        Self {
            code_hash,
            verified: status.verified,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub(super) struct VerificationSourceResponse {
    code_hash: String,
    verified: bool,
    bundle: Option<SourceBundleResponse>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub(super) struct SourceBundleResponse {
    source_bundle_hash: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    payment_tx_hash: Option<String>,
    verified_at: u64,
    storage_revision: String,
    entrypoint: String,
    compiler: CompilerResponse,
    source_map: Option<SourceMapData>,
    files: Vec<SourceFileResponse>,
}

impl From<StoredSourceBundle> for SourceBundleResponse {
    fn from(bundle: StoredSourceBundle) -> Self {
        let manifest = bundle.manifest;
        Self {
            source_bundle_hash: manifest.source_bundle_hash,
            payment_tx_hash: manifest.payment_tx_hash,
            verified_at: manifest.verified_at,
            storage_revision: bundle.storage_revision,
            entrypoint: manifest.compiler.entrypoint.clone(),
            compiler: CompilerResponse::from(manifest.compiler),
            source_map: manifest.source_map,
            files: bundle
                .files
                .into_iter()
                .map(SourceFileResponse::from)
                .collect(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub(super) struct CompilerResponse {
    language: String,
    version: String,
    #[schema(value_type = Object)]
    params: Value,
}

impl From<CompilerMetadata> for CompilerResponse {
    fn from(compiler: CompilerMetadata) -> Self {
        Self {
            language: compiler.language,
            version: compiler.version,
            params: compiler.params,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub(super) struct SourceFileResponse {
    path: String,
    content_hash: String,
    include_in_command: Option<bool>,
    is_stdlib: Option<bool>,
    has_include_directives: Option<bool>,
    content: String,
}

impl From<StoredSourceFile> for SourceFileResponse {
    fn from(file: StoredSourceFile) -> Self {
        Self {
            path: file.path,
            content_hash: file.content_hash,
            include_in_command: file.include_in_command,
            is_stdlib: file.is_stdlib,
            has_include_directives: file.has_include_directives,
            content: file.content,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub(super) struct LastVerifiedResponse {
    items: Vec<LastVerifiedItemResponse>,
    total: usize,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub(super) struct LastVerifiedItemResponse {
    code_hash: String,
    source_bundle_hash: String,
    verified_at: u64,
    storage_revision: String,
    entrypoint: String,
    compiler: CompilerResponse,
    file_count: usize,
    has_tolk_abi: bool,
    abi_name: Option<String>,
}

impl From<IndexedVerifiedBundleSummary> for LastVerifiedItemResponse {
    fn from(item: IndexedVerifiedBundleSummary) -> Self {
        Self {
            code_hash: item.code_hash,
            source_bundle_hash: item.source_bundle_hash,
            verified_at: item.verified_at,
            storage_revision: item.storage_revision,
            entrypoint: item.entrypoint,
            compiler: CompilerResponse::from(item.compiler),
            file_count: item.file_count,
            has_tolk_abi: item.has_tolk_abi,
            abi_name: item.abi_name,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub(super) struct VerificationStatisticsResponse {
    total: usize,
    languages: Vec<LanguageStatisticsResponse>,
}

impl From<VerificationStatisticsReceipt> for VerificationStatisticsResponse {
    fn from(receipt: VerificationStatisticsReceipt) -> Self {
        Self {
            total: receipt.total,
            languages: receipt
                .languages
                .into_iter()
                .map(LanguageStatisticsResponse::from)
                .collect(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub(super) struct LanguageStatisticsResponse {
    language: String,
    total: usize,
    versions: Vec<CompilerVersionStatisticsResponse>,
}

impl From<IndexedLanguageStatistics> for LanguageStatisticsResponse {
    fn from(statistics: IndexedLanguageStatistics) -> Self {
        Self {
            language: statistics.language,
            total: statistics.total,
            versions: statistics
                .versions
                .into_iter()
                .map(CompilerVersionStatisticsResponse::from)
                .collect(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub(super) struct CompilerVersionStatisticsResponse {
    version: String,
    total: usize,
}

impl From<IndexedCompilerVersionStatistics> for CompilerVersionStatisticsResponse {
    fn from(statistics: IndexedCompilerVersionStatistics) -> Self {
        Self {
            version: statistics.version,
            total: statistics.total,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub(super) struct VerificationStatisticsHistoryResponse {
    items: Vec<VerificationStatisticsHistoryItemResponse>,
}

impl From<VerificationStatisticsHistoryReceipt> for VerificationStatisticsHistoryResponse {
    fn from(receipt: VerificationStatisticsHistoryReceipt) -> Self {
        Self {
            items: receipt
                .items
                .into_iter()
                .map(VerificationStatisticsHistoryItemResponse::from)
                .collect(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub(super) struct VerificationStatisticsHistoryItemResponse {
    /// Verification time as a Unix timestamp in seconds.
    #[schema(example = 1_700_000_000)]
    timestamp: u64,
    /// Compiler identifier, such as `func`, `tact`, or `tolk`.
    #[schema(example = "tolk")]
    compiler: String,
    /// Compiler version used for the verification.
    #[schema(example = "1.4.1")]
    version: String,
}

impl From<IndexedVerificationStatisticsHistoryItem> for VerificationStatisticsHistoryItemResponse {
    fn from(item: IndexedVerificationStatisticsHistoryItem) -> Self {
        Self {
            timestamp: item.timestamp,
            compiler: item.compiler,
            version: item.version,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub(super) struct AbiContractsResponse {
    items: Vec<AbiContractResponse>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub(super) struct AbiContractResponse {
    code_hash: String,
    #[schema(value_type = Object)]
    abi: Value,
}

impl From<IndexedAbiContract> for AbiContractResponse {
    fn from(item: IndexedAbiContract) -> Self {
        Self {
            code_hash: item.code_hash,
            abi: item.abi,
        }
    }
}
