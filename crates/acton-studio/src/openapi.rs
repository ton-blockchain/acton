#![allow(dead_code, reason = "documentation-only OpenAPI models and paths")]

use std::collections::BTreeMap;

use axum::Json;
use serde_json::Value;
use utoipa::{OpenApi, ToSchema};

use crate::contract_registry::{
    ArtifactIdRequest, CodeHashRequest, ContractListEntry, DeleteContractRequest,
    RegisterCompilerAbisRequest, RegisterContractRequest, RegisterVerifiedSourcesRequest,
    SavedCompilerAbi, SavedVerifiedSource, SetAddressNameRequest,
};
use crate::{
    CreateEnvironmentConfig, CreateEnvironmentRequest, CreateEnvironmentSnapshotRequest,
    EnvironmentCapability, EnvironmentConfig, EnvironmentEndpoints, EnvironmentLifecycle,
    EnvironmentNetwork, EnvironmentSnapshot, EnvironmentSnapshotOperation,
    EnvironmentSnapshotOperationKind, EnvironmentSnapshotOperationPhase, EnvironmentStartupTimings,
    EnvironmentStatus, PublicTonNetwork, SignWalletRequest, SignWalletResponse, StudioApiErrorBody,
    StudioEnvironment, StudioInfo, StudioWallet, UpdateEnvironmentRequest, WorkspaceInfo,
};

#[utoipa::path(
    get,
    path = "/api/v1/openapi.json",
    responses((status = 200, description = "Complete OpenAPI document for the Studio control API", body = Object)),
    tag = "system"
)]
pub(crate) async fn handler() -> Json<utoipa::openapi::OpenApi> {
    Json(openapi())
}

#[must_use]
pub fn openapi() -> utoipa::openapi::OpenApi {
    let mut document = StudioApiDoc::openapi();
    document.merge(crate::test_api::openapi());
    document
}

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Acton Studio API",
        version = "1.0.0",
        description = "Control API for Acton Studio environments, wallets, contracts, and test runs. Environment RPC routes proxy the API exposed by the selected environment."
    ),
    paths(
        handler,
        crate::health,
        crate::info,
        crate::list_environments,
        crate::create_environment,
        crate::get_environment,
        crate::update_environment,
        crate::delete_environment,
        crate::stop_environment,
        crate::restart_environment,
        crate::list_environment_snapshots,
        crate::create_environment_snapshot,
        crate::restore_environment_snapshot,
        crate::delete_environment_snapshot,
        crate::get_environment_snapshot_operation,
        crate::list_wallets,
        crate::sign_wallet,
        crate::get_environment_api_calls,
        proxy_environment_rpc_root,
        proxy_environment_rpc_get,
        proxy_environment_rpc_post,
        get_address_names,
        set_address_name,
        list_contracts,
        register_contract,
        delete_contract,
        get_compiler_abis,
        list_compiler_abis,
        register_compiler_abis,
        delete_compiler_abi,
        get_registered_verified_source,
        list_verified_sources,
        register_verified_sources,
        delete_verified_source,
        delete_verified_source_artifact
    ),
    components(schemas(
        StudioApiErrorBody,
        crate::StudioApiErrorDetails,
        StudioInfo,
        WorkspaceInfo,
        CreateEnvironmentRequest,
        CreateEnvironmentConfig,
        UpdateEnvironmentRequest,
        CreateEnvironmentSnapshotRequest,
        EnvironmentConfig,
        PublicTonNetwork,
        EnvironmentCapability,
        EnvironmentEndpoints,
        EnvironmentNetwork,
        EnvironmentStatus,
        EnvironmentLifecycle,
        StudioEnvironment,
        EnvironmentSnapshot,
        EnvironmentSnapshotOperation,
        EnvironmentSnapshotOperationKind,
        EnvironmentSnapshotOperationPhase,
        EnvironmentStartupTimings,
        StudioWallet,
        SignWalletRequest,
        SignWalletResponse,
        crate::ApiCallStatus,
        crate::ApiCallType,
        crate::ApiCallSource,
        crate::ApiCallFamily,
        crate::ApiCallRecord,
        crate::ApiCallLogSnapshot,
        SetAddressNameRequest,
        RegisterContractRequest,
        DeleteContractRequest,
        RegisterCompilerAbisRequest,
        crate::contract_registry::CompilerAbiRegistration,
        CodeHashRequest,
        RegisterVerifiedSourcesRequest,
        crate::contract_registry::VerifiedSourceRegistration,
        ArtifactIdRequest,
        ContractListEntry,
        crate::contract_registry::ContractArtifact,
        crate::contract_registry::ContractSourceKind,
        SavedCompilerAbi,
        SavedVerifiedSource,
        AddressNamesResponse,
        ContractListResponse,
        ContractResponse,
        CompilerAbiMapResponse,
        CompilerAbiListResponse,
        VerifiedSourceResponse,
        VerifiedSourceListResponse,
        EmptyTonlibResponse,
        TonlibErrorResponse
    )),
    tags(
        (name = "system", description = "Studio health, version, and API schema"),
        (name = "environments", description = "Environment lifecycle"),
        (name = "wallets", description = "Environment wallets"),
        (name = "environment RPC", description = "Environment API proxy"),
        (name = "contracts", description = "Studio contract metadata facade"),
        (name = "test runs", description = "Test execution and events"),
        (name = "test artifacts", description = "Artifacts produced by test runs")
    )
)]
struct StudioApiDoc;

#[derive(ToSchema)]
#[schema(
    description = "Successful address-name lookup. The result keys are the requested addresses."
)]
struct AddressNamesResponse {
    ok: bool,
    result: BTreeMap<String, Option<String>>,
    extra: String,
}

#[derive(ToSchema)]
struct ContractListResponse {
    ok: bool,
    result: Vec<ContractListEntry>,
    extra: String,
}

#[derive(ToSchema)]
struct ContractResponse {
    ok: bool,
    result: ContractListEntry,
    extra: String,
}

#[derive(ToSchema)]
#[schema(
    description = "Successful compiler ABI lookup. The result keys are the requested code hashes."
)]
struct CompilerAbiMapResponse {
    ok: bool,
    #[schema(value_type = Object)]
    result: Value,
    extra: String,
}

#[derive(ToSchema)]
struct CompilerAbiListResponse {
    ok: bool,
    result: Vec<SavedCompilerAbi>,
    extra: String,
}

#[derive(ToSchema)]
struct VerifiedSourceResponse {
    ok: bool,
    #[schema(value_type = Object)]
    result: Value,
    extra: String,
}

#[derive(ToSchema)]
struct VerifiedSourceListResponse {
    ok: bool,
    result: Vec<SavedVerifiedSource>,
    extra: String,
}

#[derive(ToSchema)]
#[schema(description = "Successful operation with a null result.")]
struct EmptyTonlibResponse {
    ok: bool,
    #[schema(value_type = Object, nullable = true)]
    result: Value,
    extra: String,
}

#[derive(ToSchema)]
struct TonlibErrorResponse {
    ok: bool,
    error: String,
    code: i32,
    extra: String,
    jsonrpc: Option<String>,
    #[schema(value_type = Object, nullable = true)]
    id: Option<Value>,
}

#[utoipa::path(
    post,
    path = "/api/v1/environments/{environment_id}/rpc",
    params(("environment_id" = String, Path, description = "Environment ID")),
    request_body(content = Object, description = "Request for the environment control API"),
    responses(
        (status = 200, description = "Response from the environment control API", body = Object),
        (status = 404, description = "Environment not found", body = StudioApiErrorBody),
        (status = 409, description = "Environment is not running or has no control API", body = StudioApiErrorBody),
        (status = 502, description = "Failed to reach the environment API", body = StudioApiErrorBody)
    ),
    tag = "environment RPC"
)]
const fn proxy_environment_rpc_root() {}

#[utoipa::path(
    get,
    path = "/api/v1/environments/{environment_id}/rpc/{path}",
    params(
        ("environment_id" = String, Path, description = "Environment ID"),
        ("path" = String, Path, description = "Path in the environment API, for example api/v3/accountStates")
    ),
    responses(
        (status = 200, description = "Response from the selected environment API", body = Object),
        (status = 404, description = "Environment not found", body = StudioApiErrorBody),
        (status = 409, description = "Environment is not running or the selected API is unavailable", body = StudioApiErrorBody),
        (status = 502, description = "Failed to reach the environment API", body = StudioApiErrorBody)
    ),
    tag = "environment RPC"
)]
const fn proxy_environment_rpc_get() {}

#[utoipa::path(
    post,
    path = "/api/v1/environments/{environment_id}/rpc/{path}",
    params(
        ("environment_id" = String, Path, description = "Environment ID"),
        ("path" = String, Path, description = "Path in the environment API, for example api/v2/sendBoc")
    ),
    request_body(content = Object, description = "Request body defined by the selected environment API"),
    responses(
        (status = 200, description = "Response from the selected environment API", body = Object),
        (status = 404, description = "Environment not found", body = StudioApiErrorBody),
        (status = 409, description = "Environment is not running or the selected API is unavailable", body = StudioApiErrorBody),
        (status = 502, description = "Failed to reach the environment API", body = StudioApiErrorBody)
    ),
    tag = "environment RPC"
)]
const fn proxy_environment_rpc_post() {}

#[utoipa::path(
    get,
    path = "/api/v1/environments/{environment_id}/rpc/acton_getAddressName",
    params(
        ("environment_id" = String, Path, description = "Environment ID"),
        ("address" = Vec<String>, Query, description = "One or more TON addresses")
    ),
    responses(
        (status = 200, description = "Names indexed by requested address", body = AddressNamesResponse),
        (status = 409, description = "Environment is not running", body = StudioApiErrorBody),
        (status = 500, description = "Failed to read the contract registry", body = TonlibErrorResponse)
    ),
    tag = "contracts"
)]
const fn get_address_names() {}

#[utoipa::path(
    post,
    path = "/api/v1/environments/{environment_id}/rpc/acton_setAddressName",
    params(("environment_id" = String, Path, description = "Environment ID")),
    request_body = SetAddressNameRequest,
    responses(
        (status = 200, description = "Address name saved", body = EmptyTonlibResponse),
        (status = 409, description = "Environment is not running", body = StudioApiErrorBody),
        (status = 422, description = "Invalid address or request body", body = TonlibErrorResponse),
        (status = 500, description = "Failed to update the contract registry", body = TonlibErrorResponse)
    ),
    tag = "contracts"
)]
const fn set_address_name() {}

#[utoipa::path(
    get,
    path = "/api/v1/environments/{environment_id}/rpc/acton_listContracts",
    params(("environment_id" = String, Path, description = "Environment ID")),
    responses(
        (status = 200, description = "Registered active contracts", body = ContractListResponse),
        (status = 409, description = "Environment or its V3 API is unavailable", body = TonlibErrorResponse),
        (status = 502, description = "Failed to read account states", body = TonlibErrorResponse),
        (status = 500, description = "Failed to read the contract registry", body = TonlibErrorResponse)
    ),
    tag = "contracts"
)]
const fn list_contracts() {}

#[utoipa::path(
    post,
    path = "/api/v1/environments/{environment_id}/rpc/acton_registerContract",
    params(("environment_id" = String, Path, description = "Environment ID")),
    request_body = RegisterContractRequest,
    responses(
        (status = 200, description = "Registered contract", body = ContractResponse),
        (status = 409, description = "Environment or its V3 API is unavailable", body = TonlibErrorResponse),
        (status = 422, description = "Invalid address, body, or account state", body = TonlibErrorResponse),
        (status = 502, description = "Failed to read the account state", body = TonlibErrorResponse),
        (status = 500, description = "Failed to update the contract registry", body = TonlibErrorResponse)
    ),
    tag = "contracts"
)]
const fn register_contract() {}

#[utoipa::path(
    post,
    path = "/api/v1/environments/{environment_id}/rpc/acton_deleteContract",
    params(("environment_id" = String, Path, description = "Environment ID")),
    request_body = DeleteContractRequest,
    responses(
        (status = 200, description = "Contract registration deleted", body = EmptyTonlibResponse),
        (status = 422, description = "Invalid address or request body", body = TonlibErrorResponse),
        (status = 500, description = "Failed to update the contract registry", body = TonlibErrorResponse)
    ),
    tag = "contracts"
)]
const fn delete_contract() {}

#[utoipa::path(
    get,
    path = "/api/v1/environments/{environment_id}/rpc/acton_getCompilerAbi",
    params(
        ("environment_id" = String, Path, description = "Environment ID"),
        ("code_hash" = Vec<String>, Query, description = "One or more code hashes")
    ),
    responses(
        (status = 200, description = "Compiler ABIs indexed by requested code hash", body = CompilerAbiMapResponse),
        (status = 500, description = "Failed to read the contract registry", body = TonlibErrorResponse)
    ),
    tag = "contracts"
)]
const fn get_compiler_abis() {}

#[utoipa::path(
    get,
    path = "/api/v1/environments/{environment_id}/rpc/acton_listCompilerAbis",
    params(("environment_id" = String, Path, description = "Environment ID")),
    responses(
        (status = 200, description = "Registered compiler ABIs", body = CompilerAbiListResponse),
        (status = 500, description = "Failed to read the contract registry", body = TonlibErrorResponse)
    ),
    tag = "contracts"
)]
const fn list_compiler_abis() {}

#[utoipa::path(
    post,
    path = "/api/v1/environments/{environment_id}/rpc/acton_registerCompilerAbis",
    params(("environment_id" = String, Path, description = "Environment ID")),
    request_body = RegisterCompilerAbisRequest,
    responses(
        (status = 200, description = "Compiler ABIs registered", body = EmptyTonlibResponse),
        (status = 422, description = "Invalid compiler ABI registration", body = TonlibErrorResponse),
        (status = 500, description = "Failed to update the contract registry", body = TonlibErrorResponse)
    ),
    tag = "contracts"
)]
const fn register_compiler_abis() {}

#[utoipa::path(
    post,
    path = "/api/v1/environments/{environment_id}/rpc/acton_deleteCompilerAbi",
    params(("environment_id" = String, Path, description = "Environment ID")),
    request_body = CodeHashRequest,
    responses(
        (status = 200, description = "Compiler ABI deleted", body = EmptyTonlibResponse),
        (status = 422, description = "Invalid code hash or request body", body = TonlibErrorResponse),
        (status = 500, description = "Failed to update the contract registry", body = TonlibErrorResponse)
    ),
    tag = "contracts"
)]
const fn delete_compiler_abi() {}

#[utoipa::path(
    get,
    path = "/api/v1/environments/{environment_id}/rpc/acton_getRegisteredVerifiedSource",
    params(
        ("environment_id" = String, Path, description = "Environment ID"),
        ("address" = Option<String>, Query, description = "TON address used to resolve the code hash"),
        ("code_hash" = Option<String>, Query, description = "Code hash. This value takes precedence over address.")
    ),
    responses(
        (status = 200, description = "Latest registered source or an unverified result", body = VerifiedSourceResponse),
        (status = 409, description = "Environment or its V3 API is unavailable", body = TonlibErrorResponse),
        (status = 422, description = "Neither address nor code_hash is valid", body = TonlibErrorResponse),
        (status = 502, description = "Failed to resolve the account state", body = TonlibErrorResponse),
        (status = 500, description = "Failed to read the contract registry", body = TonlibErrorResponse)
    ),
    tag = "contracts"
)]
const fn get_registered_verified_source() {}

#[utoipa::path(
    get,
    path = "/api/v1/environments/{environment_id}/rpc/acton_listVerifiedSources",
    params(("environment_id" = String, Path, description = "Environment ID")),
    responses(
        (status = 200, description = "Registered verified source artifacts", body = VerifiedSourceListResponse),
        (status = 500, description = "Failed to read the contract registry", body = TonlibErrorResponse)
    ),
    tag = "contracts"
)]
const fn list_verified_sources() {}

#[utoipa::path(
    post,
    path = "/api/v1/environments/{environment_id}/rpc/acton_registerVerifiedSources",
    params(("environment_id" = String, Path, description = "Environment ID")),
    request_body = RegisterVerifiedSourcesRequest,
    responses(
        (status = 200, description = "Verified sources registered", body = EmptyTonlibResponse),
        (status = 422, description = "Invalid source registration", body = TonlibErrorResponse),
        (status = 500, description = "Failed to update the contract registry", body = TonlibErrorResponse)
    ),
    tag = "contracts"
)]
const fn register_verified_sources() {}

#[utoipa::path(
    post,
    path = "/api/v1/environments/{environment_id}/rpc/acton_deleteVerifiedSource",
    params(("environment_id" = String, Path, description = "Environment ID")),
    request_body = CodeHashRequest,
    responses(
        (status = 200, description = "Latest source registration for the code hash deleted", body = EmptyTonlibResponse),
        (status = 422, description = "Invalid code hash or request body", body = TonlibErrorResponse),
        (status = 500, description = "Failed to update the contract registry", body = TonlibErrorResponse)
    ),
    tag = "contracts"
)]
const fn delete_verified_source() {}

#[utoipa::path(
    post,
    path = "/api/v1/environments/{environment_id}/rpc/acton_deleteVerifiedSourceArtifact",
    params(("environment_id" = String, Path, description = "Environment ID")),
    request_body = ArtifactIdRequest,
    responses(
        (status = 200, description = "Source artifact registration deleted", body = EmptyTonlibResponse),
        (status = 422, description = "Invalid artifact ID or request body", body = TonlibErrorResponse),
        (status = 500, description = "Failed to update the contract registry", body = TonlibErrorResponse)
    ),
    tag = "contracts"
)]
const fn delete_verified_source_artifact() {}
