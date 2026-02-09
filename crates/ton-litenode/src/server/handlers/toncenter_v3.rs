use super::utils::handle_result;
use crate::api::toncenter_v3;
use crate::litenode::LiteNode;
use crate::server::models::{GetJettonMastersRequest, GetJettonWalletsRequest, GetTracesQuery};
use axum::{
    Json,
    extract::{Query, State},
};
use serde_json::Value;
use std::sync::Arc;
use toncenter_v3 as v3;

pub async fn get_traces(
    State(node): State<Arc<LiteNode>>,
    Query(payload): Query<GetTracesQuery>,
) -> Json<Value> {
    handle_result(node.get_traces(payload.hash), v3::map_traces).await
}

pub async fn get_jetton_masters(
    State(node): State<Arc<LiteNode>>,
    Query(payload): Query<GetJettonMastersRequest>,
) -> Json<Value> {
    handle_result(
        node.get_jetton_masters(
            payload.address,
            payload.admin_address,
            payload.limit,
            payload.offset,
        ),
        v3::map_jetton_masters,
    )
    .await
}

pub async fn get_jetton_wallets(
    State(node): State<Arc<LiteNode>>,
    Query(payload): Query<GetJettonWalletsRequest>,
) -> Json<Value> {
    handle_result(
        node.get_jetton_wallets(
            payload.address,
            payload.owner_address,
            payload.jetton_address,
            payload.exclude_zero_balance,
            payload.limit,
            payload.offset,
        ),
        v3::map_jetton_wallets,
    )
    .await
}
