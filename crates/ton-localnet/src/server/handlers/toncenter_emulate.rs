use super::toncenter_enrichment::build_extra_data_for_addresses;
use crate::api::{toncenter_emulate, toncenter_v3 as v3};
use crate::localnet::Localnet;
use crate::storage::TraceNode;
use crate::types::{Addr, BocBytes};
use axum::{
    Json,
    body::Bytes,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use std::collections::BTreeSet;
use std::sync::Arc;
use ton_api::toncenter::emulate::v1 as emulate;

macro_rules! parse {
    ($expression:expr) => {
        match $expression {
            Ok(value) => value,
            Err(error) => {
                return emulate_bad_request(format!("invalid request: {error}"));
            }
        }
    };
}

macro_rules! handle {
    ($expression:expr) => {
        match $expression {
            Ok(value) => value,
            Err(error) => return emulate_internal_error(error.to_string()),
        }
    };
}

pub async fn emulate_trace_v1(State(node): State<Arc<Localnet>>, body: Bytes) -> Response {
    let payload: emulate::EmulateRequest = parse!(serde_json::from_slice(&body));

    let boc = payload.boc;
    if boc.is_empty() {
        return emulate_bad_request("invalid request: boc is required");
    }
    parse!(BocBytes::from_base64(&boc).map_err(|error| anyhow::anyhow!("invalid boc: {error}")));

    emulate_boc_v1(
        node.as_ref(),
        boc,
        payload.ignore_chksig,
        payload.mc_block_seqno,
        EmulateResponseOptions {
            include_code_data: payload.include_code_data,
            include_address_book: payload.include_address_book,
            include_metadata: payload.include_metadata,
            with_actions: payload.with_actions,
        },
    )
    .await
}

pub async fn emulate_ton_connect_v1(State(node): State<Arc<Localnet>>, body: Bytes) -> Response {
    let payload: emulate::TonConnectEmulateRequest = parse!(serde_json::from_slice(&body));
    parse!(toncenter_emulate::validate_ton_connect_request(&payload));

    let account = handle!(
        node.get_address_information(payload.from.clone(), payload.mc_block_seqno)
            .await
    );
    let now = handle!(node.clock_info().await).current_unix_time;
    let boc = handle!(toncenter_emulate::compose_ton_connect_message(
        &payload, &account, now
    ))
    .to_base64();

    emulate_boc_v1(
        node.as_ref(),
        boc,
        true,
        payload.mc_block_seqno,
        EmulateResponseOptions {
            include_code_data: payload.include_code_data,
            include_address_book: payload.include_address_book,
            include_metadata: payload.include_metadata,
            with_actions: payload.with_actions,
        },
    )
    .await
}

#[derive(Clone, Copy)]
struct EmulateResponseOptions {
    include_code_data: bool,
    include_address_book: bool,
    include_metadata: bool,
    with_actions: bool,
}

async fn emulate_boc_v1(
    node: &Localnet,
    boc: String,
    ignore_chksig: bool,
    mc_block_seqno: Option<u32>,
    options: EmulateResponseOptions,
) -> Response {
    let trace = handle!(
        node.emulate_trace(boc, Some(ignore_chksig), mc_block_seqno)
            .await
    );
    let (address_book, metadata) = handle!(
        build_emulate_v1_extra_data(
            node,
            &trace.trace,
            options.include_address_book,
            options.include_metadata,
        )
        .await
    );

    let response = v3::map_emulate_trace_response(
        &trace,
        options.with_actions,
        options.include_code_data,
        address_book,
        metadata,
    );
    (StatusCode::OK, Json(response)).into_response()
}

async fn build_emulate_v1_extra_data(
    node: &Localnet,
    trace: &TraceNode,
    include_address_book: bool,
    include_metadata: bool,
) -> anyhow::Result<(
    Option<ton_api::toncenter::v3::AddressBook>,
    Option<ton_api::toncenter::v3::Metadata>,
)> {
    if !include_address_book && !include_metadata {
        return Ok((None, None));
    }

    let mut addresses = BTreeSet::new();
    collect_trace_addresses(trace, &mut addresses);
    build_extra_data_for_addresses(
        node,
        addresses.into_iter().collect(),
        include_address_book,
        include_metadata,
    )
    .await
}

fn collect_trace_addresses(trace: &TraceNode, out: &mut BTreeSet<Addr>) {
    out.insert(trace.transaction.meta.account);
    if let Some(in_msg) = &trace.transaction.in_msg {
        out.extend(in_msg.meta.src);
        out.extend(in_msg.meta.dst);
    }
    for out_msg in &trace.transaction.out_msgs {
        out.extend(out_msg.meta.src);
        out.extend(out_msg.meta.dst);
    }
    for child in &trace.children {
        collect_trace_addresses(child, out);
    }
}

fn emulate_bad_request(error: impl Into<String>) -> Response {
    emulate_error_response(StatusCode::BAD_REQUEST, error)
}

fn emulate_internal_error(error: impl Into<String>) -> Response {
    emulate_error_response(StatusCode::INTERNAL_SERVER_ERROR, error)
}

fn emulate_error_response(status: StatusCode, error: impl Into<String>) -> Response {
    (
        status,
        Json(emulate::ErrorResponse {
            error: error.into(),
        }),
    )
        .into_response()
}
