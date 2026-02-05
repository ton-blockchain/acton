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
        .route(
            "/v2/getAddressInformation",
            get(get_address_information_query).post(get_address_information_post),
        )
        .route(
            "/v2/getAddressBalance",
            get(get_address_balance_query).post(get_address_balance_post),
        )
        .route(
            "/v2/getAddressState",
            get(get_address_state_query).post(get_address_state_post),
        )
        .route(
            "/v2/getExtendedAddressInformation",
            get(get_extended_address_information_query).post(get_extended_address_information_post),
        )
        .route(
            "/v2/getTransactions",
            get(get_transactions_query).post(get_transactions_post),
        )
        .route(
            "/v2/getBlockHeader",
            get(get_block_header_query).post(get_block_header_post),
        )
        .route(
            "/v2/getBlockTransactions",
            get(get_block_transactions_query).post(get_block_transactions_post),
        )
        .route(
            "/v2/getBlockTransactionsExt",
            get(get_block_transactions_ext_query).post(get_block_transactions_ext_post),
        )
        .route(
            "/v2/getMasterchainInfo",
            get(get_masterchain_info_query).post(get_masterchain_info_post),
        )
        .route("/v2/getShards", get(get_shards_query).post(get_shards_post))
        .route("/v2/shards", get(get_shards_query).post(get_shards_post))
        .route(
            "/v2/lookupBlock",
            get(lookup_block_query).post(lookup_block_post),
        );

    let api_v3_router = Router::new().route("/v3/traces", get(get_traces_query));

    Router::new()
        .nest("/api", api_v2_router)
        .nest("/api", api_v3_router)
        .route("/faucet", post(faucet))
        .route(
            "/state-source",
            get(get_state_source).post(set_state_source),
        )
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(node)
}
