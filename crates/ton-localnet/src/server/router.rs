use super::handlers::utils::get_extra;
use super::handlers::*;
use crate::localnet::Localnet;
use axum::{
    Json, Router,
    http::{HeaderValue, Method, StatusCode, request::Parts},
    response::{IntoResponse, Response},
    routing::{get, post},
};
#[cfg(not(debug_assertions))]
use include_dir::{Dir, include_dir};
use serde_json::json;
#[cfg(debug_assertions)]
use std::path::PathBuf;
use std::sync::Arc;
use tower_governor::governor::GovernorConfigBuilder;
use tower_governor::key_extractor::GlobalKeyExtractor;
use tower_governor::{GovernorError, GovernorLayer};
use tower_http::cors::{AllowOrigin, Any, CorsLayer};
#[cfg(debug_assertions)]
use tower_http::services::{ServeDir, ServeFile};
use tower_http::trace::TraceLayer;

#[cfg(not(debug_assertions))]
static UI_DIR: Dir<'static> = include_dir!("$CARGO_MANIFEST_DIR/../acton-localnet-ui/dist");

pub fn create_router(node: Arc<Localnet>, rate_limit_rps: Option<u32>) -> Router {
    let api_v2_router = Router::new()
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
        .route("/v2/getAddressBalance", get(get_address_balance))
        .route("/v2/getAddressState", get(get_address_state))
        .route("/v2/getLibraries", get(get_libraries))
        .route(
            "/v2/getExtendedAddressInformation",
            get(get_extended_address_information),
        )
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

    let api_v3_router = Router::new()
        .route("/v3/traces", get(get_traces))
        .route("/v3/accountStates", get(get_account_states_v3))
        .route("/v3/addressInformation", get(get_address_information_v3))
        .route("/v3/transactions", get(get_transactions_v3))
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

    let emulate_router = Router::new().route("/emulate/v1/emulateTrace", post(emulate_trace_v1));

    let mut api_router = Router::new()
        .merge(api_v2_router)
        .merge(api_v3_router)
        .merge(emulate_router);
    let admin_router = Router::new()
        .route("/faucet", post(faucet))
        .route(
            "/address-name",
            get(get_address_name).post(set_address_name),
        )
        .route("/compiler-abi", get(get_compiler_abi))
        .route("/compiler-abis", post(register_compiler_abis))
        .route(
            "/state-source",
            get(get_state_source).post(set_state_source),
        );

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

    let app = Router::new()
        .nest("/api", api_router)
        .nest("/admin", admin_router)
        .layer(loopback_cors())
        .layer(TraceLayer::new_for_http())
        .with_state(node);

    #[cfg(debug_assertions)]
    let app = {
        let dist_path = PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../acton-localnet-ui/dist"
        ));
        app.fallback_service(
            ServeDir::new(&dist_path).fallback(ServeFile::new(dist_path.join("index.html"))),
        )
    };

    #[cfg(not(debug_assertions))]
    let app = app.fallback(handle_embedded_ui);

    app
}

fn loopback_cors() -> CorsLayer {
    CorsLayer::new()
        .allow_origin(AllowOrigin::predicate(
            |origin: &HeaderValue, _request_parts: &Parts| is_loopback_origin(origin),
        ))
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers(Any)
}

fn is_loopback_origin(origin: &HeaderValue) -> bool {
    let Ok(origin) = origin.to_str() else {
        return false;
    };
    let Ok(uri) = origin.parse::<axum::http::Uri>() else {
        return false;
    };
    matches!(uri.scheme_str(), Some("http" | "https")) && uri.host().is_some_and(is_loopback_host)
}

fn is_loopback_host(host: &str) -> bool {
    host.eq_ignore_ascii_case("localhost")
        || host == "127.0.0.1"
        || host == "::1"
        || host == "[::1]"
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

#[cfg(not(debug_assertions))]
async fn handle_embedded_ui(uri: axum::http::Uri) -> impl IntoResponse {
    let path = uri.path().trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };

    if let Some(file) = UI_DIR.get_file(path) {
        let content_type = match path.split('.').next_back() {
            Some("html") => "text/html",
            Some("js") => "application/javascript",
            Some("css") => "text/css",
            Some("svg") => "image/svg+xml",
            Some("png") => "image/png",
            Some("json") => "application/json",
            _ => "application/octet-stream",
        };
        return (([("content-type", content_type)]), file.contents()).into_response();
    }

    if let Some(index) = UI_DIR.get_file("index.html") {
        return (([("content-type", "text/html")]), index.contents()).into_response();
    }

    StatusCode::NOT_FOUND.into_response()
}
