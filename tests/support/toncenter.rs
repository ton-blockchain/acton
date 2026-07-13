#![allow(dead_code)]

use crate::common::strip_ansi;
use crate::support::TestOutputExt;
use crate::support::localnet::LocalnetHandle;
use crate::support::project::{ActonCommand, Project, ProjectBuilder};
use std::fmt::Write as _;
use std::fs;
use std::io::{BufRead, BufReader, ErrorKind, Read, Write};
use std::net::TcpListener;
use std::path::Path;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use ton::ton_core::types::TonAddress;
use ton_localnet::types::Addr;
use tvm_ffi::json_stack::legacy_stack_to_json;
use tvm_ffi::stack::{Tuple, TupleItem};
use tycho_types::boc::Boc;
use tycho_types::cell::HashBytes;
use tycho_types::cell::{Cell, CellBuilder, CellFamily, Store};
use tycho_types::dict::{Dict, RawDict};
use tycho_types::models::{IntAddr, ShardAccount, StdAddr};

#[derive(Clone)]
pub(crate) struct ToncenterV2MockResponse {
    pub(crate) status: u16,
    pub(crate) body: String,
}

#[derive(Clone)]
pub(crate) struct ToncenterV3MockResponse {
    pub(crate) status: u16,
    pub(crate) body: String,
}

#[derive(Debug, Clone)]
pub(crate) struct CapturedToncenterRequest {
    pub(crate) method: String,
    pub(crate) path: String,
    pub(crate) headers: Vec<(String, String)>,
    pub(crate) body: Vec<u8>,
}

pub(crate) const DEPLOYER_WALLET_CONFIG: &str = r#"[wallets.deployer]
kind = "v4r2"
workchain = 0
keys = { mnemonic = "cupboard match uphold miracle fog balance unknown region share hand trophy million toy narrow ability exchange first toast fresh maid report cram strong later" }
"#;

pub(crate) fn spawn_toncenter_v2_mock(
    responses: Vec<ToncenterV2MockResponse>,
) -> (String, thread::JoinHandle<()>) {
    let (url, handle, _) = spawn_toncenter_v2_mock_with_capture(responses);
    (url, handle)
}

pub(crate) fn spawn_toncenter_v2_mock_with_capture(
    responses: Vec<ToncenterV2MockResponse>,
) -> (
    String,
    thread::JoinHandle<()>,
    Arc<Mutex<Vec<CapturedToncenterRequest>>>,
) {
    spawn_toncenter_mock_with_capture(
        responses
            .into_iter()
            .map(|response| (response.status, response.body))
            .collect(),
    )
}

pub(crate) fn spawn_toncenter_v3_mock(
    responses: Vec<ToncenterV3MockResponse>,
) -> (
    String,
    thread::JoinHandle<()>,
    Arc<Mutex<Vec<CapturedToncenterRequest>>>,
) {
    spawn_toncenter_mock_with_capture(
        responses
            .into_iter()
            .map(|response| (response.status, response.body))
            .collect(),
    )
}

pub(crate) fn spawn_toncenter_mock_with_capture(
    responses: Vec<(u16, String)>,
) -> (
    String,
    thread::JoinHandle<()>,
    Arc<Mutex<Vec<CapturedToncenterRequest>>>,
) {
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("failed to bind toncenter v2 mock");
    listener
        .set_nonblocking(true)
        .expect("failed to set toncenter v2 mock non-blocking");
    let addr = listener
        .local_addr()
        .expect("failed to get toncenter v2 mock address");

    let captured_requests = Arc::new(Mutex::new(Vec::<CapturedToncenterRequest>::new()));
    let captured_requests_thread = Arc::clone(&captured_requests);

    let handle = thread::spawn(move || {
        for (status, body) in responses {
            let wait_until = Instant::now() + Duration::from_secs(30);
            let mut stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(err) if err.kind() == ErrorKind::WouldBlock => {
                        assert!(
                            Instant::now() <= wait_until,
                            "timed out waiting for toncenter v2 request"
                        );
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(err) => panic!("toncenter v2 mock accept failed: {err}"),
                }
            };

            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .expect("failed to set toncenter v2 mock read timeout");

            let mut reader = BufReader::new(
                stream
                    .try_clone()
                    .expect("failed to clone toncenter v2 mock stream"),
            );
            let mut request_line = String::new();
            let read_deadline = Instant::now() + Duration::from_secs(2);
            loop {
                request_line.clear();
                match reader.read_line(&mut request_line) {
                    Ok(0) => {
                        assert!(
                            Instant::now() <= read_deadline,
                            "timed out waiting for toncenter v2 request line"
                        );
                        thread::sleep(Duration::from_millis(10));
                    }
                    Ok(_) => break,
                    Err(err)
                        if matches!(err.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) =>
                    {
                        assert!(
                            Instant::now() <= read_deadline,
                            "timed out waiting for toncenter v2 request line"
                        );
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(err) => panic!("failed to read toncenter v2 request line: {err}"),
                }
            }

            let mut parts = request_line.split_whitespace();
            let method = parts.next().unwrap_or_default().to_string();
            let path = parts.next().unwrap_or_default().to_string();

            let mut headers = Vec::new();
            let mut content_length = 0_usize;
            loop {
                let mut header_line = String::new();
                let read = reader
                    .read_line(&mut header_line)
                    .expect("failed to read toncenter v2 header line");
                if read == 0 || header_line == "\r\n" {
                    break;
                }

                if let Some((name, value)) = header_line.split_once(':')
                    && name.trim().eq_ignore_ascii_case("content-length")
                {
                    content_length = value.trim().parse().unwrap_or(0);
                }

                if let Some((name, value)) = header_line.split_once(':') {
                    headers.push((name.trim().to_string(), value.trim().to_string()));
                }
            }

            let mut request_body = Vec::new();
            if content_length > 0 {
                request_body.resize(content_length, 0);
                reader
                    .read_exact(&mut request_body)
                    .expect("failed to read toncenter v2 request body");
            }

            captured_requests_thread
                .lock()
                .expect("captured toncenter requests mutex poisoned")
                .push(CapturedToncenterRequest {
                    method,
                    path,
                    headers,
                    body: request_body,
                });

            let raw_response = format!(
                "HTTP/1.1 {} {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                status,
                status_text(status),
                body.len(),
                body
            );
            stream
                .write_all(raw_response.as_bytes())
                .expect("failed to write toncenter v2 response");
            stream
                .flush()
                .expect("failed to flush toncenter v2 response");
        }
    });

    (format!("http://{addr}"), handle, captured_requests)
}

pub(crate) fn append_custom_network(project_path: &Path, network_name: &str, v2_url: &str) {
    let acton_toml_path = project_path.join("Acton.toml");
    let mut acton_toml =
        fs::read_to_string(&acton_toml_path).expect("failed to read generated Acton.toml");
    let _ = write!(
        acton_toml,
        r#"

[networks.{network_name}]
api = {{ v2 = "{v2_url}" }}
"#
    );
    fs::write(&acton_toml_path, acton_toml)
        .expect("failed to write Acton.toml with custom network");
}

pub(crate) fn append_custom_network_with_urls(
    project_path: &Path,
    network_name: &str,
    v2_url: &str,
    v3_url: &str,
) {
    let acton_toml_path = project_path.join("Acton.toml");
    let mut acton_toml =
        fs::read_to_string(&acton_toml_path).expect("failed to read generated Acton.toml");
    let _ = write!(
        acton_toml,
        r#"

[networks.{network_name}]
api = {{ v2 = "{v2_url}", v3 = "{v3_url}" }}
"#
    );
    fs::write(&acton_toml_path, acton_toml)
        .expect("failed to write Acton.toml with custom network");
}

pub(crate) fn append_localnet_network(project_path: &Path, v2_url: &str) {
    let acton_toml_path = project_path.join("Acton.toml");
    let mut acton_toml =
        fs::read_to_string(&acton_toml_path).expect("failed to read generated Acton.toml");
    let _ = write!(
        acton_toml,
        r#"

[networks.localnet]
api = {{ v2 = "{v2_url}" }}
"#
    );
    fs::write(&acton_toml_path, acton_toml)
        .expect("failed to write Acton.toml with localnet network");
}

pub(crate) fn append_localnet_with_base_url(project_path: &Path, base_url: &str) {
    append_custom_network_with_urls(
        project_path,
        "localnet",
        &format!("{base_url}/api/v2"),
        &format!("{base_url}/api/v3"),
    );
}

pub(crate) fn jetton_v1_action_project(name: &str) -> Project {
    ProjectBuilder::new(name)
        .contract_from_boc_with_types(
            "JettonV1Master",
            include_bytes!(
                "../integration/testdata/toncenter_v3_actions/contracts/JettonV1Master.boc"
            )
            .to_vec(),
            "types/jetton_v1_master.types.tolk",
        )
        .contract_from_boc_with_types(
            "JettonV1Wallet",
            include_bytes!(
                "../integration/testdata/toncenter_v3_actions/contracts/JettonV1Wallet.boc"
            )
            .to_vec(),
            "types/jetton_v1_wallet.types.tolk",
        )
        .file(
            "types/jetton_v1_master.types",
            include_str!(
                "../integration/testdata/toncenter_v3_actions/types/jetton_v1_master.types.tolk"
            ),
        )
        .file(
            "types/jetton_v1_wallet.types",
            include_str!(
                "../integration/testdata/toncenter_v3_actions/types/jetton_v1_wallet.types.tolk"
            ),
        )
        .file(
            "wrappers/JettonV1Master.gen",
            include_str!(
                "../integration/testdata/toncenter_v3_actions/wrappers/JettonV1Master.gen.tolk"
            ),
        )
        .file(
            "wrappers/JettonV1Wallet.gen",
            include_str!(
                "../integration/testdata/toncenter_v3_actions/wrappers/JettonV1Wallet.gen.tolk"
            ),
        )
        .script_file(
            "jetton",
            include_str!("../integration/testdata/toncenter_v3_actions/jetton.tolk"),
        )
        .mapping("@acton", "../lib")
        .build()
}

pub(crate) fn with_nft_v1_action_fixtures(project: ProjectBuilder) -> ProjectBuilder {
    project
        .contract_from_boc_with_types(
            "NftV1Collection",
            include_bytes!(
                "../integration/testdata/toncenter_v3_actions/contracts/NftV1Collection.boc"
            )
            .to_vec(),
            "types/nft_v1_collection.types.tolk",
        )
        .contract_from_boc_with_types(
            "NftV1Item",
            include_bytes!("../integration/testdata/toncenter_v3_actions/contracts/NftV1Item.boc")
                .to_vec(),
            "types/nft_v1_item.types.tolk",
        )
        .file(
            "types/nft_v1_collection.types",
            include_str!(
                "../integration/testdata/toncenter_v3_actions/types/nft_v1_collection.types.tolk"
            ),
        )
        .file(
            "types/nft_v1_item.types",
            include_str!(
                "../integration/testdata/toncenter_v3_actions/types/nft_v1_item.types.tolk"
            ),
        )
        .file(
            "wrappers/NftV1Collection.gen",
            include_str!(
                "../integration/testdata/toncenter_v3_actions/wrappers/NftV1Collection.gen.tolk"
            ),
        )
        .file(
            "wrappers/NftV1Item.gen",
            include_str!(
                "../integration/testdata/toncenter_v3_actions/wrappers/NftV1Item.gen.tolk"
            ),
        )
}

pub(crate) fn nft_v1_action_project(name: &str) -> Project {
    with_nft_v1_action_fixtures(ProjectBuilder::new(name))
        .script_file(
            "nft",
            include_str!("../integration/testdata/toncenter_v3_actions/nft.tolk"),
        )
        .mapping("@acton", "../lib")
        .build()
}

pub(crate) fn run_localnet_action_project(
    project: &Project,
    script_path: &str,
) -> (LocalnetHandle, String) {
    fs::write(project.path().join("wallets.toml"), DEPLOYER_WALLET_CONFIG)
        .expect("Failed to write wallets.toml");
    let node = project
        .localnet()
        .before_start(ActonCommand::build)
        .args(["--accounts", "deployer"])
        .start();
    append_localnet_with_base_url(project.path(), &node.base_url());
    let output = project
        .acton()
        .script(script_path)
        .verify_network("localnet")
        .run()
        .success()
        .get_stdout();
    (node, output)
}

pub(crate) fn extract_canonical_addr_marker(output: &str, marker: &str) -> String {
    let cleaned = strip_ansi(output);
    let value = cleaned
        .lines()
        .map(str::trim)
        .find_map(|line| line.strip_prefix(marker))
        .unwrap_or_else(|| panic!("Marker `{marker}` not found in output:\n{cleaned}"));
    let address = value.split_once(" (").map_or(value, |(address, _)| address);
    Addr::parse(address)
        .unwrap_or_else(|error| panic!("Invalid address `{value}` printed for {marker}: {error}"))
        .to_string()
}

pub(crate) fn toncenter_v2_seqno_ok_response() -> ToncenterV2MockResponse {
    toncenter_v2_run_get_method_ok_response(vec![TupleItem::Int(0.into())], 0)
}

pub(crate) fn toncenter_v2_run_get_method_ok_response(
    stack: Vec<TupleItem>,
    exit_code: i32,
) -> ToncenterV2MockResponse {
    ToncenterV2MockResponse {
        status: 200,
        body: serde_json::json!({
            "ok": true,
            "@extra": "0",
            "result": {
                "@type": "smc.runResult",
                "gas_used": "0",
                "stack": legacy_stack_to_json(&Tuple(stack)).expect("stack must serialize to legacy json"),
                "exit_code": exit_code,
                "block_id": {
                    "@type": "ton.blockIdExt",
                    "workchain": -1,
                    "shard": "-9223372036854775808",
                    "seqno": 0,
                    "root_hash": "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=",
                    "file_hash": "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA="
                },
                "last_transaction_id": {
                    "@type": "internal.transactionId",
                    "lt": "0",
                    "hash": "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA="
                }
            }
        })
        .to_string(),
    }
}

pub(crate) fn toncenter_v2_account_info_ok_response(
    balance: i64,
    state: &str,
    lt: u64,
    hash: &str,
) -> ToncenterV2MockResponse {
    ToncenterV2MockResponse {
        status: 200,
        body: serde_json::json!({
            "ok": true,
            "@extra": "0",
            "result": {
                "@type": "raw.fullAccountState",
                "balance": balance.to_string(),
                "extra_currencies": [],
                "code": "",
                "data": "",
                "state": state,
                "frozen_hash": "",
                "last_transaction_id": {
                    "@type": "internal.transactionId",
                    "lt": lt.to_string(),
                    "hash": hash,
                },
                "block_id": {
                    "@type": "ton.blockIdExt",
                    "workchain": -1,
                    "shard": "-9223372036854775808",
                    "seqno": 0,
                    "root_hash": "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=",
                    "file_hash": "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA="
                },
                "sync_utime": 0,
                "suspended": false
            }
        })
        .to_string(),
    }
}

pub(crate) fn toncenter_v2_account_info_with_code_ok_response(
    balance: i64,
    code_boc64: &str,
    data_boc64: &str,
    state: &str,
    frozen_hash: &str,
    lt: &str,
    hash: &str,
) -> ToncenterV2MockResponse {
    ToncenterV2MockResponse {
        status: 200,
        body: serde_json::json!({
            "ok": true,
            "@extra": "0",
            "result": {
                "@type": "raw.fullAccountState",
                "balance": balance.to_string(),
                "extra_currencies": [],
                "code": code_boc64,
                "data": data_boc64,
                "state": state,
                "frozen_hash": frozen_hash,
                "last_transaction_id": {
                    "@type": "internal.transactionId",
                    "lt": lt,
                    "hash": hash,
                },
                "block_id": {
                    "@type": "ton.blockIdExt",
                    "workchain": -1,
                    "shard": "-9223372036854775808",
                    "seqno": 0,
                    "root_hash": "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=",
                    "file_hash": "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA="
                },
                "sync_utime": 0,
                "suspended": false
            }
        })
        .to_string(),
    }
}

pub(crate) fn toncenter_v2_masterchain_info_ok_response(seqno: u64) -> ToncenterV2MockResponse {
    ToncenterV2MockResponse {
        status: 200,
        body: serde_json::json!({
            "ok": true,
            "@extra": "0",
            "result": {
                "@type": "blocks.masterchainInfo",
                "last": {
                    "@type": "ton.blockIdExt",
                    "workchain": -1,
                    "shard": "-9223372036854775808",
                    "seqno": seqno,
                    "root_hash": "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=",
                    "file_hash": "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA="
                },
                "state_root_hash": "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=",
                "init": {
                    "@type": "ton.blockIdExt",
                    "workchain": -1,
                    "shard": "-9223372036854775808",
                    "seqno": 0,
                    "root_hash": "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=",
                    "file_hash": "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA="
                }
            }
        })
        .to_string(),
    }
}

pub(crate) fn toncenter_v2_shard_account_cell_ok_response(
    shard_account: &ShardAccount,
) -> ToncenterV2MockResponse {
    ToncenterV2MockResponse {
        status: 200,
        body: serde_json::json!({
            "ok": true,
            "@extra": "0",
            "result": {
                "@type": "tvm.cell",
                "bytes": Boc::encode_base64(to_cell(shard_account))
            }
        })
        .to_string(),
    }
}

pub(crate) fn format_captured_requests(requests: &[CapturedToncenterRequest]) -> String {
    let mut out = String::new();
    for request in requests {
        let _ = writeln!(out, "{} {}", request.method, request.path);
    }
    out
}

pub(crate) fn write_fork_account_cache_summary(
    project_path: &Path,
    network_name: &str,
    fork_block_number: u64,
    output_file_name: &str,
    requests: &[CapturedToncenterRequest],
) {
    let mut out = String::new();
    out.push_str("requests:\n");
    let formatted_requests = format_captured_requests(requests);
    if formatted_requests.is_empty() {
        out.push_str("<none>\n");
    } else {
        out.push_str(&formatted_requests);
    }

    out.push_str("cache_files:\n");
    let cache_dir = project_path
        .join("build")
        .join("cache")
        .join(network_name)
        .join(fork_block_number.to_string());
    match fs::read_dir(cache_dir) {
        Ok(entries) => {
            let mut file_names = entries
                .map(|entry| {
                    entry
                        .expect("failed to read fork account cache directory entry")
                        .file_name()
                        .to_string_lossy()
                        .into_owned()
                })
                .collect::<Vec<_>>();
            file_names.sort();
            if file_names.is_empty() {
                out.push_str("<empty>\n");
            } else {
                for file_name in file_names {
                    let _ = writeln!(out, "{file_name}");
                }
            }
        }
        Err(err) if err.kind() == ErrorKind::NotFound => out.push_str("<missing>\n"),
        Err(err) => panic!("failed to read fork account cache directory: {err}"),
    }

    fs::write(project_path.join(output_file_name), out)
        .expect("failed to write fork account cache summary");
}

pub(crate) fn write_fork_account_cache_tree_summary(
    project_path: &Path,
    network_name: &str,
    output_file_name: &str,
    requests: &[CapturedToncenterRequest],
) {
    let mut out = String::new();
    out.push_str("requests:\n");
    let formatted_requests = format_captured_requests(requests);
    if formatted_requests.is_empty() {
        out.push_str("<none>\n");
    } else {
        out.push_str(&formatted_requests);
    }

    out.push_str("cache_tree:\n");
    let cache_root = project_path.join("build").join("cache").join(network_name);
    match collect_cache_tree_entries(&cache_root) {
        Ok(entries) if entries.is_empty() => out.push_str("<empty>\n"),
        Ok(entries) => {
            for entry in entries {
                let _ = writeln!(out, "{entry}");
            }
        }
        Err(err) if err.kind() == ErrorKind::NotFound => out.push_str("<missing>\n"),
        Err(err) => panic!("failed to read fork account cache tree: {err}"),
    }

    fs::write(project_path.join(output_file_name), out)
        .expect("failed to write fork account cache tree summary");
}

fn collect_cache_tree_entries(root: &Path) -> std::io::Result<Vec<String>> {
    fn collect(
        root: &Path,
        relative_prefix: &Path,
        entries: &mut Vec<String>,
    ) -> std::io::Result<()> {
        let mut children = fs::read_dir(root)?.collect::<Result<Vec<_>, _>>()?;
        children.sort_by_key(fs::DirEntry::file_name);

        for child in children {
            let child_name = child.file_name();
            let child_relative = relative_prefix.join(&child_name);
            let file_type = child.file_type()?;
            if file_type.is_dir() {
                entries.push(format!("{}/", child_relative.to_string_lossy()));
                collect(&child.path(), &child_relative, entries)?;
            } else if file_type.is_file() {
                entries.push(child_relative.to_string_lossy().into_owned());
            }
        }

        Ok(())
    }

    let mut entries = Vec::new();
    collect(root, Path::new(""), &mut entries)?;
    Ok(entries)
}

pub(crate) fn toncenter_v2_verify_registry_address_response(
    registry_address: &str,
) -> ToncenterV2MockResponse {
    toncenter_v2_run_get_method_ok_response(
        vec![TupleItem::Cell(to_cell(&ton_address_to_std_addr(
            &TonAddress::from_str(registry_address).expect("registry address must parse"),
        )))],
        0,
    )
}

pub(crate) fn toncenter_v2_verify_quorum_response(
    verifier_id: &str,
    quorum: u8,
) -> ToncenterV2MockResponse {
    let verifier_entry = build_verifier_registry_entry_cell(verifier_id, quorum);
    let mut dict = Dict::<HashBytes, tycho_types::cell::CellSlice>::new();
    let value = verifier_entry
        .as_slice()
        .expect("verifier entry cell must convert to slice");
    dict.add(HashBytes([0x11; 32]), value)
        .expect("verifier dict entry must be added");

    toncenter_v2_run_get_method_ok_response(vec![TupleItem::Cell(to_cell(&dict))], 0)
}

pub(crate) fn toncenter_v2_error_response(status: u16, error: &str) -> ToncenterV2MockResponse {
    ToncenterV2MockResponse {
        status,
        body: serde_json::json!({
            "ok": false,
            "error": error,
            "code": i32::from(status),
            "@extra": "0"
        })
        .to_string(),
    }
}

pub(crate) fn toncenter_v2_send_boc_ok_response() -> ToncenterV2MockResponse {
    ToncenterV2MockResponse {
        status: 200,
        body: serde_json::json!({
            "ok": true,
            "@extra": "0",
            "result": {"@type": "ok"}
        })
        .to_string(),
    }
}

pub(crate) fn toncenter_v2_send_boc_error_response(error: &str) -> ToncenterV2MockResponse {
    ToncenterV2MockResponse {
        status: 500,
        body: serde_json::json!({
            "ok": false,
            "error": error,
            "code": 500,
            "@extra": "0"
        })
        .to_string(),
    }
}

pub(crate) fn toncenter_v2_send_boc_client_error_response(error: &str) -> ToncenterV2MockResponse {
    ToncenterV2MockResponse {
        status: 400,
        body: serde_json::json!({
            "ok": false,
            "error": error,
            "code": 400,
            "@extra": "0"
        })
        .to_string(),
    }
}

pub(crate) fn toncenter_v2_get_libraries_ok_response(data: &str) -> ToncenterV2MockResponse {
    ToncenterV2MockResponse {
        status: 200,
        body: serde_json::json!({
            "ok": true,
            "@extra": "0",
            "result": {
                "@type": "smc.libraryResult",
                "result": [{
                    "@type": "smc.libraryEntry",
                    "hash": "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=",
                    "data": data
                }]
            }
        })
        .to_string(),
    }
}

pub(crate) fn toncenter_v3_account_states_ok_response(
    address: &str,
    code_boc: Option<&str>,
    status: &str,
) -> ToncenterV3MockResponse {
    ToncenterV3MockResponse {
        status: 200,
        body: serde_json::json!({
            "accounts": [{
                "address": address,
                "account_state_hash": "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=",
                "balance": "0",
                "code_boc": code_boc,
                "status": status
            }]
        })
        .to_string(),
    }
}

pub(crate) fn toncenter_v3_error_response(status: u16, error: &str) -> ToncenterV3MockResponse {
    ToncenterV3MockResponse {
        status,
        body: serde_json::json!({
            "error": error
        })
        .to_string(),
    }
}

fn status_text(status: u16) -> &'static str {
    match status {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        429 => "Too Many Requests",
        500 => "Internal Server Error",
        _ => "Unknown",
    }
}

fn to_cell<T: Store + ?Sized>(obj: &T) -> Cell {
    let mut builder = CellBuilder::new();
    obj.store_into(&mut builder, Cell::empty_context())
        .expect("failed to store object into cell");
    builder.build().expect("failed to build cell")
}

fn ton_address_to_std_addr(address: &TonAddress) -> StdAddr {
    StdAddr {
        anycast: None,
        address: HashBytes(
            <[u8; 32]>::try_from(address.hash.as_slice())
                .expect("TonAddress hash must be exactly 32 bytes"),
        ),
        workchain: address.workchain as i8,
    }
}

fn build_verifier_registry_entry_cell(verifier_id: &str, quorum: u8) -> Cell {
    let mut builder = CellBuilder::new();
    IntAddr::Std(StdAddr::new(0, HashBytes([0; 32])))
        .store_into(&mut builder, Cell::empty_context())
        .expect("admin address must store");
    builder.store_u8(quorum).expect("quorum must store");
    RawDict::<256>::from(None)
        .store_into(&mut builder, Cell::empty_context())
        .expect("empty endpoint dict must store");
    builder
        .store_reference(build_snake_string_cell(verifier_id))
        .expect("verifier id must store");
    builder
        .store_reference(build_snake_string_cell("https://verifier.invalid"))
        .expect("verifier url must store");
    builder.build().expect("verifier entry cell must build")
}

fn build_snake_string_cell(text: &str) -> Cell {
    let bytes = text.as_bytes();
    let total_bits = bytes.len() * 8;
    let mut builder = CellBuilder::new();
    builder
        .store_raw(bytes, total_bits as u16)
        .expect("snake string bytes must store");
    builder.build().expect("snake string cell must build")
}
