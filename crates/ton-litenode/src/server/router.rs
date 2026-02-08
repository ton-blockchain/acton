use super::handlers::*;
use crate::litenode::LiteNode;
use axum::{
    Router,
    routing::{get, post},
};
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

pub fn create_router(node: Arc<LiteNode>) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let api_v2_router = Router::new()
        .route("/v2", post(json_rpc))
        .route("/v2/jsonRPC", post(json_rpc))
        .route("/v2/v2/jsonRPC", post(json_rpc))
        .route("/v2/sendBoc", post(send_boc))
        .route("/v2/sendBocReturnHash", post(send_boc_return_hash))
        .route("/v2/runGetMethod", post(run_get_method))
        .route("/v2/runGetMethodStd", post(run_get_method_std))
        .route("/v2/getAddressInformation", get(get_address_information))
        .route("/v2/getAddressBalance", get(get_address_balance))
        .route("/v2/getAddressState", get(get_address_state))
        .route(
            "/v2/getExtendedAddressInformation",
            get(get_extended_address_information),
        )
        .route("/v2/getTransactions", get(get_transactions))
        .route("/v2/getTransactionsBySource", get(get_transactions_by_source))
        .route("/v2/getBlockHeader", get(get_block_header))
        .route("/v2/getBlockTransactions", get(get_block_transactions))
        .route(
            "/v2/getBlockTransactionsExt",
            get(get_block_transactions_ext),
        )
        .route("/v2/getMasterchainInfo", get(get_masterchain_info))
        .route("/v2/getShards", get(get_shards))
        .route("/v2/shards", get(get_shards))
        .route("/v2/lookupBlock", get(lookup_block));

    let api_v3_router = Router::new().route("/v3/traces", get(get_traces));

    Router::new()
        .nest("/api", api_v2_router)
        .nest("/api", api_v3_router)
        .route("/faucet", post(faucet))
        .route("/address-name", get(get_address_name).post(set_address_name))
        .route(
            "/state-source",
            get(get_state_source).post(set_state_source),
        )
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(node)
}
