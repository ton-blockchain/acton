use super::handlers::utils::get_extra;
use super::handlers::{
    change_account_state, create_recovery_point, detect_address, detect_hash, dump_state,
    emulate_trace_v1, export_recovery_point, faucet, get_account_states_v3, get_address_balance,
    get_address_information, get_address_information_v3, get_address_name, get_address_state,
    get_api_calls, get_block_header, get_block_transactions, get_block_transactions_ext,
    get_blocks_v3, get_compiler_abi, get_config_all, get_config_param, get_consensus_block,
    get_extended_address_information, get_jetton_masters, get_jetton_wallets, get_libraries,
    get_masterchain_info, get_nft_items, get_out_msg_queue_size, get_pending_transactions_v3,
    get_shard_account_cell, get_shards, get_startup_wallets, get_status, get_token_data,
    get_traces, get_transactions, get_transactions_by_message_v3, get_transactions_std,
    get_transactions_v3, get_verified_source, get_wallet_information, import_recovery_point,
    increase_time, json_rpc, list_recovery_points, load_state, lookup_block, mine_blocks,
    pack_address, register_compiler_abis, revert_recovery_point, run_get_method,
    run_get_method_std, run_get_method_v3, send_boc, send_boc_return_hash, send_internal_message,
    send_message_v3, set_address_name, set_mining_mode, set_network_conditions,
    set_next_block_timestamp, set_shard_account, set_time, streaming_sse, streaming_ws,
    try_locate_result_tx, try_locate_source_tx, try_locate_tx, unpack_address,
};
use crate::server::{
    ApiCallAlreadyRecorded, ApiCallFamily, ApiCallInput, ApiCallLog, ApiCallType,
    NetworkConditions, ServerState,
};
use axum::{
    Json, Router,
    extract::Request,
    http::{Method, StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
};
#[cfg(not(debug_assertions))]
use include_dir::{Dir, include_dir};
use serde_json::{Value, json};
#[cfg(debug_assertions)]
use std::fs;
#[cfg(debug_assertions)]
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use tower_governor::governor::GovernorConfigBuilder;
use tower_governor::key_extractor::GlobalKeyExtractor;
use tower_governor::{GovernorError, GovernorLayer};
use tower_http::compression::CompressionLayer;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

#[cfg(not(debug_assertions))]
static UI_DIR: Dir<'static> = include_dir!("$CARGO_MANIFEST_DIR/../acton-localnet-ui/dist");

pub fn create_router(state: ServerState, rate_limit_rps: Option<u32>) -> Router {
    let mut api_v2_router = Router::new()
        .route("/v2", post(json_rpc))
        .route("/v2/jsonRPC", post(json_rpc))
        .route("/v2/v2/jsonRPC", post(json_rpc))
        .route("/v2/sendBoc", post(send_boc))
        .route("/v2/sendBocReturnHash", post(send_boc_return_hash))
        .route("/v2/runGetMethod", post(run_get_method))
        .route("/v2/runGetMethodStd", post(run_get_method_std))
        .route("/v2/detectAddress", get(detect_address))
        .route("/v2/detectHash", get(detect_hash))
        .route("/v2/packAddress", get(pack_address))
        .route("/v2/unpackAddress", get(unpack_address))
        .route("/v2/getAddressInformation", get(get_address_information))
        .route("/v2/getShardAccountCell", get(get_shard_account_cell))
        .route("/v2/getAddressBalance", get(get_address_balance))
        .route("/v2/getAddressState", get(get_address_state))
        .route("/v2/getLibraries", get(get_libraries))
        .route(
            "/v2/getExtendedAddressInformation",
            get(get_extended_address_information),
        )
        .route("/v2/getWalletInformation", get(get_wallet_information))
        .route("/v2/getTokenData", get(get_token_data))
        .route("/v2/getTransactions", get(get_transactions))
        .route("/v2/getTransactionsStd", get(get_transactions_std))
        .route("/v2/tryLocateTx", get(try_locate_tx))
        .route("/v2/tryLocateResultTx", get(try_locate_result_tx))
        .route("/v2/tryLocateSourceTx", get(try_locate_source_tx))
        .route("/v2/getConfigParam", get(get_config_param))
        .route("/v2/getConfigAll", get(get_config_all))
        .route("/v2/getBlockHeader", get(get_block_header))
        .route("/v2/getBlockTransactions", get(get_block_transactions))
        .route(
            "/v2/getBlockTransactionsExt",
            get(get_block_transactions_ext),
        )
        .route("/v2/getMasterchainInfo", get(get_masterchain_info))
        .route("/v2/getConsensusBlock", get(get_consensus_block))
        .route("/v2/getOutMsgQueueSize", get(get_out_msg_queue_size))
        .route("/v2/getShards", get(get_shards))
        .route("/v2/shards", get(get_shards))
        .route("/v2/lookupBlock", get(lookup_block));

    let mut api_v3_router = Router::new()
        .route("/v3/traces", get(get_traces))
        .route("/v3/accountStates", get(get_account_states_v3))
        .route("/v3/addressInformation", get(get_address_information_v3))
        .route("/v3/transactions", get(get_transactions_v3))
        .route("/v3/blocks", get(get_blocks_v3))
        .route(
            "/v3/transactionsByMessage",
            get(get_transactions_by_message_v3),
        )
        .route("/v3/pendingTransactions", get(get_pending_transactions_v3))
        .route("/v3/message", post(send_message_v3))
        .route("/v3/runGetMethod", post(run_get_method_v3))
        .route("/v3/jetton/masters", get(get_jetton_masters))
        .route("/v3/jetton/wallets", get(get_jetton_wallets))
        .route("/v3/nft/items", get(get_nft_items));

    let mut emulate_router =
        Router::new().route("/emulate/v1/emulateTrace", post(emulate_trace_v1));
    let streaming_router = Router::new()
        .route("/streaming/v2/sse", post(streaming_sse))
        .route("/streaming/v2/ws", get(streaming_ws));

    let api_v2_conditions = state.network_conditions.clone();
    api_v2_router = api_v2_router.layer(middleware::from_fn(move |request, next| {
        delay_response(request, next, api_v2_conditions.clone())
    }));
    let api_v3_conditions = state.network_conditions.clone();
    api_v3_router = api_v3_router.layer(middleware::from_fn(move |request, next| {
        delay_response(request, next, api_v3_conditions.clone())
    }));
    let emulate_conditions = state.network_conditions.clone();
    emulate_router = emulate_router.layer(middleware::from_fn(move |request, next| {
        delay_response(request, next, emulate_conditions.clone())
    }));

    let mut api_router = Router::new()
        .merge(api_v2_router)
        .merge(api_v3_router)
        .merge(emulate_router)
        .merge(streaming_router);
    let api_calls_for_admin = state.api_calls.clone();
    let acton_router = Router::new()
        .route("/acton_fundAccount", post(faucet))
        .route("/acton_getAddressName", get(get_address_name))
        .route("/acton_setAddressName", post(set_address_name))
        .route("/acton_getCompilerAbi", get(get_compiler_abi))
        .route("/acton_getVerifiedSource", get(get_verified_source))
        .route("/acton_registerCompilerAbis", post(register_compiler_abis))
        .route("/acton_dumpState", post(dump_state))
        .route("/acton_loadState", post(load_state))
        .route("/acton_snapshot", post(create_recovery_point))
        .route("/acton_listSnapshots", post(list_recovery_points))
        .route("/acton_revert", post(revert_recovery_point))
        .route("/acton_exportSnapshot", post(export_recovery_point))
        .route("/acton_importSnapshot", post(import_recovery_point))
        .route("/acton_setShardAccount", post(set_shard_account))
        .route("/acton_changeAccountState", post(change_account_state))
        .route("/acton_sendInternalMessage", post(send_internal_message))
        .route("/acton_getStartupWallets", get(get_startup_wallets))
        .route("/acton_setNetworkConditions", post(set_network_conditions))
        .route("/acton_setMiningMode", post(set_mining_mode))
        .route("/acton_mine", post(mine_blocks))
        .route("/acton_increaseTime", post(increase_time))
        .route("/acton_setTime", post(set_time))
        .route(
            "/acton_setNextBlockTimestamp",
            post(set_next_block_timestamp),
        )
        .route("/acton_getApiCalls", get(get_api_calls))
        .route("/acton_nodeInfo", get(get_status))
        .layer(middleware::from_fn(move |request, next| {
            record_api_call(request, next, api_calls_for_admin.clone())
        }));

    if let Some(limit) = rate_limit_rps {
        let mut governor_config = GovernorConfigBuilder::default();
        governor_config.per_second(1).burst_size(limit);
        let mut governor_config = governor_config.key_extractor(GlobalKeyExtractor);
        let governor_config = Arc::new(
            governor_config
                .finish()
                .expect("Rate limit configuration must be valid"),
        );
        let governor_layer = GovernorLayer::new(governor_config)
            .error_handler(move |error| governor_error_response(error, limit));
        api_router = api_router.layer(governor_layer);
    }

    let api_calls_for_api = state.api_calls.clone();
    api_router = api_router.layer(middleware::from_fn(move |request, next| {
        record_api_call(request, next, api_calls_for_api.clone())
    }));

    let mut api_entry_router = Router::new().nest("/api", api_router);
    let mut control_router = acton_router;
    if let Some(auth_token) = state.auth_token.clone() {
        let api_auth_token = auth_token.clone();
        api_entry_router = api_entry_router.layer(middleware::from_fn(move |request, next| {
            require_auth(request, next, api_auth_token.clone())
        }));
        control_router = control_router.layer(middleware::from_fn(move |request, next| {
            require_auth(request, next, auth_token.clone())
        }));
    }
    api_entry_router = api_entry_router.layer(public_api_cors());

    let protected_router = Router::new().merge(api_entry_router).merge(control_router);

    let app = Router::new()
        .merge(protected_router)
        .fallback(handle_embedded_ui)
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    app.layer(CompressionLayer::new())
}

async fn delay_response(request: Request, next: Next, conditions: NetworkConditions) -> Response {
    let response = next.run(request).await;
    let delay_ms = conditions.response_delay_ms();
    if delay_ms > 0 {
        sleep(Duration::from_millis(delay_ms)).await;
    }
    response
}

async fn require_auth(request: Request, next: Next, auth_token: Arc<str>) -> Response {
    if request.method() == Method::OPTIONS || request_has_valid_auth(&request, &auth_token) {
        return next.run(request).await;
    }

    (
        StatusCode::UNAUTHORIZED,
        Json(json!({
            "ok": false,
            "error": "Unauthorized: missing or invalid localnet API token",
            "code": 401,
            "@extra": get_extra()
        })),
    )
        .into_response()
}

fn request_has_valid_auth(request: &Request, auth_token: &str) -> bool {
    bearer_token_matches(request, auth_token)
        || api_key_matches(request, auth_token)
        || websocket_query_token_matches(request, auth_token)
}

fn bearer_token_matches(request: &Request, auth_token: &str) -> bool {
    request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        == Some(auth_token)
}

fn api_key_matches(request: &Request, auth_token: &str) -> bool {
    request
        .headers()
        .get("x-api-key")
        .and_then(|value| value.to_str().ok())
        == Some(auth_token)
}

fn websocket_query_token_matches(request: &Request, auth_token: &str) -> bool {
    let path = request
        .uri()
        .path()
        .strip_prefix("/api")
        .unwrap_or_else(|| request.uri().path());
    if path != "/streaming/v2/ws" {
        return false;
    }

    request.uri().query().is_some_and(|query| {
        url::form_urlencoded::parse(query.as_bytes())
            .any(|(key, value)| key == "token" && value == auth_token)
    })
}

async fn record_api_call(request: Request, next: Next, api_calls: ApiCallLog) -> Response {
    let start = ApiCallLog::start();
    let http_method = request.method().as_str().to_owned();
    let path = request.uri().path().to_owned();
    let response = next.run(request).await;

    if response
        .extensions()
        .get::<ApiCallAlreadyRecorded>()
        .is_some()
    {
        return response;
    }

    let status_code = response.status().as_u16();
    if let Some(mut input) = api_call_input(&http_method, &path) {
        input.status_code = status_code;
        api_calls.record(input, start);
    }

    response
}

fn api_call_input(http_method: &str, path: &str) -> Option<ApiCallInput> {
    let normalized_api_path = path.strip_prefix("/api").unwrap_or(path);
    let (api_family, method) = if path.starts_with("/acton_") {
        if matches!(path, "/acton_getApiCalls" | "/acton_nodeInfo") {
            return None;
        }
        (
            ApiCallFamily::Control,
            path.trim_start_matches('/').to_owned(),
        )
    } else if normalized_api_path == "/v2"
        || normalized_api_path == "/v2/jsonRPC"
        || normalized_api_path == "/v2/v2/jsonRPC"
    {
        (ApiCallFamily::JsonRpc, "jsonRPC".to_owned())
    } else if normalized_api_path.starts_with("/v2/") {
        (
            ApiCallFamily::V2,
            normalized_api_path
                .trim_start_matches("/v2/")
                .split('/')
                .next()
                .unwrap_or("v2")
                .to_owned(),
        )
    } else if normalized_api_path.starts_with("/v3/") {
        (
            ApiCallFamily::V3,
            normalized_api_path
                .trim_start_matches("/v3/")
                .split('/')
                .next()
                .unwrap_or("v3")
                .to_owned(),
        )
    } else if normalized_api_path.starts_with("/emulate/") {
        (
            ApiCallFamily::Emulate,
            normalized_api_path
                .trim_start_matches('/')
                .split('/')
                .next_back()
                .unwrap_or("emulate")
                .to_owned(),
        )
    } else if normalized_api_path.starts_with("/streaming/") {
        (
            ApiCallFamily::Streaming,
            normalized_api_path.trim_start_matches('/').to_owned(),
        )
    } else {
        return None;
    };

    Some(ApiCallInput {
        call_type: classify_http_call_type(http_method, &method, normalized_api_path),
        api_family,
        http_method: http_method.to_owned(),
        path: path.to_owned(),
        method,
        request_id: Value::Null,
        status_code: 0,
    })
}

fn classify_http_call_type(http_method: &str, method: &str, path: &str) -> ApiCallType {
    if matches!(http_method, "GET" | "HEAD" | "OPTIONS") {
        return ApiCallType::Read;
    }

    let method = method.to_ascii_lowercase();
    let path = path.to_ascii_lowercase();
    if method.contains("rungetmethod")
        || method.starts_with("get")
        || method.starts_with("detect")
        || method.contains("packaddress")
        || path.starts_with("/streaming/")
        || path.starts_with("/emulate/")
    {
        ApiCallType::Read
    } else {
        ApiCallType::Write
    }
}

fn public_api_cors() -> CorsLayer {
    CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers(Any)
}

fn governor_error_response(error: GovernorError, max_requests_per_second: u32) -> Response {
    match error {
        GovernorError::TooManyRequests { wait_time, headers } => {
            let mut response = (
                StatusCode::TOO_MANY_REQUESTS,
                Json(json!({
                    "ok": false,
                    "error": format!(
                        "Rate limit exceeded: max {} request(s) per second (retry in {}s)",
                        max_requests_per_second, wait_time
                    ),
                    "code": 429,
                    "@extra": get_extra()
                })),
            )
                .into_response();
            if let Some(headers) = headers {
                response.headers_mut().extend(headers);
            }
            response
        }
        GovernorError::UnableToExtractKey => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "ok": false,
                "error": "Rate limiter was unable to extract request key",
                "code": 500,
                "@extra": get_extra()
            })),
        )
            .into_response(),
        GovernorError::Other { code, msg, headers } => {
            let mut response = (
                code,
                Json(json!({
                    "ok": false,
                    "error": msg.unwrap_or_else(|| "Rate limiter error".to_string()),
                    "code": code.as_u16(),
                    "@extra": get_extra()
                })),
            )
                .into_response();
            if let Some(headers) = headers {
                response.headers_mut().extend(headers);
            }
            response
        }
    }
}

async fn handle_embedded_ui(uri: axum::http::Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };

    if let Some(contents) = load_ui_file(path) {
        return (([("content-type", ui_content_type(path))]), contents).into_response();
    }

    if let Some(index) = load_ui_file("index.html") {
        return (([("content-type", "text/html")]), index).into_response();
    }

    StatusCode::NOT_FOUND.into_response()
}

fn ui_content_type(path: &str) -> &'static str {
    match path.split('.').next_back() {
        Some("html") => "text/html",
        Some("js") => "application/javascript",
        Some("css") => "text/css",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("json") => "application/json",
        _ => "application/octet-stream",
    }
}

#[cfg(debug_assertions)]
fn load_ui_file(path: &str) -> Option<Vec<u8>> {
    if path.contains("..") {
        return None;
    }
    let dist_path = PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../acton-localnet-ui/dist"
    ));
    let file_path = dist_path.join(path);
    if !file_path.is_file() {
        return None;
    }
    fs::read(file_path).ok()
}

#[cfg(not(debug_assertions))]
fn load_ui_file(path: &str) -> Option<Vec<u8>> {
    UI_DIR.get_file(path).map(|file| file.contents().to_vec())
}
