use crate::common::assertion;
use crate::support::localnet::{LocalnetHandle, pretty_json_for_snapshot};
use crate::support::project::ProjectBuilder;
use crate::support::toncenter::{
    active_shard_account_boc64, summarize_v2_account_state, test_std_addr,
};
use serde_json::{Value, json};
use ton_api::toncenter::v2::responses as v2;
use tycho_types::boc::Boc;
use tycho_types::cell::{Cell, CellBuilder};

// Compiled contract BOCs are pinned to external/ton af7b55483a18037e4e7b5e56b06e83289e5a83da.
const HIGHLOAD_V1_CODE: &str = "te6cckEBCAEAlwABFP8A9KQT9LzyyAsBAgEgAgMCAUgEBQC48oMI1xgg0x/TH9MfAvgju/Jj7UTQ0x/TH9P/0VEyuvKhUUS68qIE+QFUEFX5EPKj9ATR+AB/jhYhgBD0eG+lIJgC0wfUMAH7AJEy4gGz5lsBpMjLH8sfy//J7VQABNAwAgFIBgcAF7s5ztRNDTPzHXC/+AARuMl+1E0NcLH4vWoNMQ==";
const HIGHLOAD_V2_R2_CODE: &str = "te6cckEBCQEA5QABFP8A9KQT9LzyyAsBAgEgAgMCAUgEBQHq8oMI1xgg0x/TP/gjqh9TILnyY+1E0NMf0z/T//QE0VNggED0Dm+hMfJgUXO68qIH+QFUEIf5EPKjAvQE0fgAf44WIYAQ9HhvpSCYAtMH1DAB+wCRMuIBs+ZbgyWhyEA0gED0Q4rmMcgSyx8Tyz/L//QAye1UCAAE0DACASAGBwAXvZznaiaGmvmOuF/8AEG+X5dqJoaY+Y6Z/p/5j6AmipEEAgegc30JjJLb/JXdHxQANCCAQPSWb6UyURCUMFMDud4gkzM2AZIyMOKzkNcT9w==";
const MANUAL_DNS_R1_CODE: &str = "te6cckECGAEAAtAAART/APSkE/S88sgLAQIBIAIDAgFIBAUC7PLbPAWDCNcYIPkBAdMf0z/4I6ofUyC58mNTKoBA9A5voTHyYFKUuvKiVBNG+RDyo/gAItcLBcAzmDQBdtch0/8wjoVa2zxAA+IDgyWhyEAHgED0Q44aIIBA9JZvpTJREJQwUwe53iCTMzUBkjIw4rPmNVUD8AQREgICxQYHAgEgDA0CAc8ICQAIqoJfAwIBSAoLACHWQK5Y+J5Z/l//oAegBk9qpAAFF8DgABcyPQAydBBM/Rw8qGAAF72c52omhpr5jrhf/AIBIA4PABG7Nz7UTQ1wsfgD+7owwh10kglF8DcG3hIHew8l4ieNci1wsHnnDIUATPFhPLB8nQAqYI3iDACJRfA3Bt4Ns8FF8EI3ADqwKY0wcBwAAToQLkIG2OnF8DIcjLBiTPFsnQhAlUQgHbPAWlFbIgwQEVQzDmMzUilF8FcG3hMgHHAJMxfwHfAtdJpvmBEVEAAYIcAAkjEB4AKAEPRqABztRNDTH9M/0//0BPQE0QE2cFmOlNs8IMcBnCDXSpPUMNCTMn8C4t4i5jAxEwT20wUhwQqOLCGRMeEhwAGXMdMH1AL7AOABwAmOFNQh+wTtQwLQ7R7tU1RiA/EGgvIA4PIt4HAiwRSUMNIPAd5tbSTBHoreJMEUjpElhAkj2zwzApUyxwDyo5Fb4t4kwAuOEzQC9ARQJIAQ9G4wECOECVnwAQHgJMAMiuAwFBUWFwCEMQLTAAHAAZPUAdCY0wUBqgLXGAHiINdJwg/ypiB41yLXCwfyaHBTEddJqTYCmNMHAcAAEqEB5DDIywYBzxbJ0FADACBZ9KhvpSCUAvQEMJIybeICACg0A4AQ9FqZECOECUBE8AEBkjAx4gBmMSLAFZwy9AQQI4QJUELwAQHgIsAWmDIChAn0czAB4DAyIMAfkzD0BODAIJJtAeDyLG0BbHytww==";

#[test]
fn specialized_extended_account_states_match_toncenter_shapes() {
    let project = ProjectBuilder::new("localnet-v2-specialized-account-states").build();
    let node = project.localnet().start();

    let highload_v1 = set_active_account(
        &node,
        0x51,
        HIGHLOAD_V1_CODE,
        Some(data_cell(&[0x8000_0001, 0xfedc_ba98])),
    );
    let highload_v2 = set_active_account(
        &node,
        0x52,
        HIGHLOAD_V2_R2_CODE,
        Some(data_cell(&[0xfedc_ba98])),
    );
    let highload_v2_without_data = set_active_account(&node, 0x53, HIGHLOAD_V2_R2_CODE, None);
    let dns = set_active_account(
        &node,
        0x54,
        MANUAL_DNS_R1_CODE,
        Some(data_cell(&[0x8000_0000])),
    );
    let dns_without_data = set_active_account(&node, 0x55, MANUAL_DNS_R1_CODE, None);

    let malformed = set_active_account(&node, 0x56, HIGHLOAD_V1_CODE, Some(data_cell(&[123])));
    let (malformed_status, malformed_body) = node.get_json_with_status(&format!(
        "/api/v2/getExtendedAddressInformation?address={malformed}"
    ));

    let snapshot = json!({
        "highload_v1_revision_minus_one": extended_state_summary(&node, &highload_v1),
        "highload_v2_revision_two": extended_state_summary(&node, &highload_v2),
        "highload_v2_without_data": extended_state_summary(&node, &highload_v2_without_data),
        "manual_dns_revision_one": extended_state_summary(&node, &dns),
        "manual_dns_without_data": extended_state_summary(&node, &dns_without_data),
        "known_code_with_truncated_data": {
            "status": malformed_status,
            "ok": malformed_body["ok"],
            "error": malformed_body["error"],
        },
    });
    assertion().eq(
        pretty_json_for_snapshot(&snapshot, project.path()),
        snapbox::file!("snapshots/v2_specialized_account_states.json"),
    );

    node.stop();
}

fn set_active_account(
    node: &LocalnetHandle,
    address_byte: u8,
    code_boc64: &str,
    data: Option<Cell>,
) -> String {
    let address = test_std_addr(address_byte);
    let raw_address = format!("0:{}", hex::encode(address.address.0));
    let code = Boc::decode_base64(code_boc64).expect("fixture code BOC must decode");
    node.post_json(
        "/acton_setShardAccount",
        &json!({
            "address": raw_address,
            "shard_account": active_shard_account_boc64(address, code, data, 5_000_000_000),
        }),
    );
    raw_address
}

fn data_cell(words: &[u32]) -> Cell {
    let mut builder = CellBuilder::new();
    for word in words {
        builder
            .store_u32(*word)
            .expect("fixture data word must fit");
    }
    builder.build().expect("fixture data cell must build")
}

fn extended_state_summary(node: &LocalnetHandle, address: &str) -> Value {
    let extended: v2::TonlibResponse<v2::ExtendedAddressInformation> = node.get_json_as(&format!(
        "/api/v2/getExtendedAddressInformation?address={address}"
    ));
    let wallet: v2::TonlibResponse<v2::WalletInformation> =
        node.get_json_as(&format!("/api/v2/getWalletInformation?address={address}"));

    json!({
        "revision": extended.result.revision,
        "state": summarize_v2_account_state(&extended.result.account_state),
        "get_wallet_information_is_wallet": wallet.result.wallet,
    })
}
