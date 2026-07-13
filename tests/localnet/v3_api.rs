use crate::common::assertion;
use crate::support::localnet::pretty_json_for_snapshot;
use crate::support::project::ProjectBuilder;
use serde_json::{Value, json};
use ton_api::toncenter::v3::{requests, responses};

const ZERO_ADDRESS: &str = "0:0000000000000000000000000000000000000000000000000000000000000000";

#[test]
fn collection_endpoints_deserialize_empty_responses() {
    let project = ProjectBuilder::new("localnet-v3-empty-collection-responses").build();
    let node = project.localnet().start();

    let pending_actions: responses::ActionsResponse =
        node.get_json_as(&format!("/api/v3/pendingActions?account={ZERO_ADDRESS}"));
    let pending_traces: responses::TracesResponse =
        node.get_json_as(&format!("/api/v3/pendingTraces?account={ZERO_ADDRESS}"));
    let traces: responses::TracesResponse =
        node.get_json_as(&format!("/api/v3/traces?account={ZERO_ADDRESS}"));
    let dns: responses::DnsRecordsResponse =
        node.get_json_as("/api/v3/dns/records?domain=missing.ton");
    let multisig_orders: responses::MultisigOrdersResponse =
        node.get_json_as(&format!("/api/v3/multisig/orders?address={ZERO_ADDRESS}"));
    let multisigs: responses::MultisigsResponse = node.get_json_as(&format!(
        "/api/v3/multisig/wallets?address={ZERO_ADDRESS}&include_orders=false"
    ));
    let vesting: responses::VestingContractsResponse =
        node.get_json_as(&format!("/api/v3/vesting?contract_address={ZERO_ADDRESS}"));

    let summary = json!({
        "pending_actions": {
            "count": pending_actions.actions.len(),
            "address_book_count": pending_actions.address_book.len(),
            "metadata_count": pending_actions.metadata.len(),
        },
        "pending_traces": {
            "count": pending_traces.traces.len(),
            "address_book_count": pending_traces.address_book.len(),
            "metadata_count": pending_traces.metadata.len(),
        },
        "traces": {
            "count": traces.traces.len(),
            "address_book_count": traces.address_book.len(),
            "metadata_count": traces.metadata.len(),
        },
        "dns_record_count": dns.records.len(),
        "multisig_order_count": multisig_orders.orders.len(),
        "multisig_count": multisigs.multisigs.len(),
        "vesting_count": vesting.vesting_contracts.len(),
    });

    assertion().eq(
        pretty_json_for_snapshot(&summary, project.path()),
        snapbox::file!("snapshots/v3_empty_collection_responses.json"),
    );

    node.stop();
}

#[test]
fn filter_and_stack_validation_returns_typed_errors() {
    let project = ProjectBuilder::new("localnet-v3-validation-errors").build();
    let node = project.localnet().start();

    let mut summary = Vec::new();
    for (case, path) in [
        ("pending actions without filter", "/api/v3/pendingActions"),
        ("pending traces without filter", "/api/v3/pendingTraces"),
        ("traces without filter", "/api/v3/traces"),
        ("dns without filter", "/api/v3/dns/records"),
        ("multisig orders without filter", "/api/v3/multisig/orders"),
        (
            "multisig wallets without filter",
            "/api/v3/multisig/wallets",
        ),
    ] {
        let (status, error): (u16, responses::RequestError) = node.get_json_with_status_as(path);
        summary.push(json!({
            "case": case,
            "status": status,
            "code": error.code,
            "error": error.error,
        }));
    }

    for (case, stack_entry) in [
        (
            "unsupported stack type",
            requests::StackEntry {
                kind: "unsupported".to_owned(),
                value: Value::Null,
            },
        ),
        (
            "cell without bytes",
            requests::StackEntry {
                kind: "cell".to_owned(),
                value: json!({}),
            },
        ),
        (
            "tuple without elements",
            requests::StackEntry {
                kind: "tuple".to_owned(),
                value: json!({}),
            },
        ),
    ] {
        let request = requests::RunGetMethodRequest {
            address: ZERO_ADDRESS.to_owned(),
            method: "seqno".to_owned(),
            stack: vec![stack_entry],
        };
        let (status, error): (u16, responses::RequestError) =
            node.post_json_with_status_as("/api/v3/runGetMethod", &request);
        summary.push(json!({
            "case": case,
            "status": status,
            "code": error.code,
            "error": error.error,
        }));
    }

    assertion().eq(
        pretty_json_for_snapshot(&Value::Array(summary), project.path()),
        snapbox::file!("snapshots/v3_validation_errors.json"),
    );

    node.stop();
}
