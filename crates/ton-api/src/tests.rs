use super::*;
use std::ffi::OsStr;
use std::sync::Arc;

#[tokio::test(flavor = "current_thread")]
async fn blocking_client_supports_headers_queries_and_clones_inside_tokio() {
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let server = std::thread::spawn(move || {
        let mut captured = Vec::new();
        for result in [r#""42""#, r#""43""#, "[]"] {
            let (mut stream, _) = listener.accept().unwrap();
            let mut bytes = Vec::new();
            let mut buffer = [0; 2048];
            while !bytes.windows(4).any(|value| value == b"\r\n\r\n") {
                let count = stream.read(&mut buffer).unwrap();
                assert_ne!(count, 0);
                bytes.extend_from_slice(&buffer[..count]);
            }
            captured.push(String::from_utf8(bytes).unwrap());
            let body = if result == "[]" {
                "{\"accounts\":[],\"address_book\":{}}".to_owned()
            } else {
                format!("{{\"ok\":true,\"@extra\":\"\",\"result\":{result}}}")
            };
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
        }
        captured
    });
    let networks = HashMap::from([(
        "localnet".to_owned(),
        CustomNetworkUrls {
            v2_url: format!("{endpoint}/prefix/v2").into(),
            v3_url: Some(format!("{endpoint}/prefix/v3").into()),
            explorer_url: None,
        },
    )]);
    let client = TonApiClient::with_options(
        Network::Localnet,
        networks,
        toncenter_client::Client::builder()
            .api_key(Some("fixture-key".to_owned()))
            .origin("https://app.example")
            .user_agent("acton-test/1")
            .operation_timeout(Duration::from_secs(2)),
    )
    .unwrap();
    let first = client.get_address_balance("0:a").unwrap();
    let cloned = client.clone();
    drop(client);
    let second = cloned.get_address_balance("0:b").unwrap();
    let accounts = cloned.get_account_states(&["0:a", "0:b"]).unwrap();
    assert!(cloned.has_api_key());
    drop(cloned);
    let requests = server.join().unwrap();
    let routes: Vec<_> = requests
        .iter()
        .map(|request| request.lines().next().unwrap())
        .collect();
    expect_test::expect![[r#"
        (
            "42",
            "43",
            0,
            [
                "GET /prefix/v2/getAddressBalance?address=0%3Aa HTTP/1.1",
                "GET /prefix/v2/getAddressBalance?address=0%3Ab HTTP/1.1",
                "GET /prefix/v3/accountStates?address=0%3Aa&address=0%3Ab HTTP/1.1",
            ],
            true,
        )
    "#]]
    .assert_debug_eq(&(
        first.to_string(),
        second.to_string(),
        accounts.len(),
        routes,
        requests.iter().all(|request| {
            request.contains("x-api-key: fixture-key")
                && request.contains("origin: https://app.example")
                && request.contains("user-agent: acton-test/1")
        }),
    ));
}

#[test]
fn acton_use_proxy_is_disabled_by_default() {
    assert!(!proxy_enabled_from_value(None));
}

#[test]
fn acton_use_proxy_accepts_1_or_true() {
    for value in ["1", "true"] {
        assert!(proxy_enabled_from_value(Some(OsStr::new(value))));
    }
}

#[test]
fn acton_use_proxy_rejects_other_values() {
    for value in ["", "0", "false", "TRUE", "yes"] {
        assert!(!proxy_enabled_from_value(Some(OsStr::new(value))));
    }
}

#[test]
fn normalize_toncenter_error_message_maps_missing_account_state() {
    assert_eq!(
        normalize_toncenter_error_message(
            "cannot apply external message to current state : Failed to unpack account state",
        ),
        Some(
            "external message not accepted because account has no state; check if wallet/contract is deployed",
        ),
    );
}

#[test]
fn normalize_toncenter_error_message_maps_pre_execution_wallet_rejection() {
    assert_eq!(
        normalize_toncenter_error_message(
            "cannot apply external message to current state : External message was not accepted: cannot run message on account: inbound external message rejected by account 3029B3EAEDA86A5381D86100F2A8B761C38DE45642EDB6E4BB1CCA2E6DD7FFED before smart-contract execution",
        ),
        Some(
            r"wallet/contract rejected the external message before contract execution; likely causes:
- not enough balance
- wallet/contract is not deployed
- seqno is stale
- message expired",
        ),
    );
}

#[test]
fn normalize_toncenter_error_message_preserves_other_errors() {
    assert_eq!(
        normalize_toncenter_error_message("mock toncenter failure"),
        None,
    );
}

#[test]
fn masterchain_snapshot_cache_round_trips() {
    let temp_dir = tempfile::tempdir().expect("temporary cache directory");
    let network = Network::Custom(Arc::from("snapshot-test"));
    let snapshot = MasterchainSnapshot {
        seqno: 123_456,
        gen_utime: 1_700_000_000,
        config: Cell::default(),
    };
    let entry = MasterchainSnapshotCacheEntry::new(
        &network,
        "http://127.0.0.1:8080/api/v2".to_owned(),
        &snapshot,
        Some(snapshot.gen_utime),
        100,
    );
    let path = masterchain_snapshot_cache_path(temp_dir.path(), &network, snapshot.seqno);

    write_masterchain_snapshot_cache(&path, &entry).expect("write snapshot cache");
    let cached = read_masterchain_snapshot_cache(
        &path,
        &network,
        "http://127.0.0.1:8080/api/v2",
        snapshot.seqno,
    )
    .expect("read snapshot cache");

    assert_eq!(cached.gen_utime, Some(snapshot.gen_utime));
    assert_eq!(cached.config.repr_hash(), snapshot.config.repr_hash());
    let latest_cached = MasterchainSnapshotCacheEntry::new(
        &network,
        "http://127.0.0.1:8080/api/v2".to_owned(),
        &snapshot,
        None,
        100,
    )
    .into_cached_snapshot()
    .expect("decode latest snapshot cache");
    assert_eq!(latest_cached.gen_utime, None);
    assert!(
        read_masterchain_snapshot_cache(
            &path,
            &network,
            "http://127.0.0.1:8081/api/v2",
            snapshot.seqno,
        )
        .is_none(),
        "cache from another endpoint must not be reused"
    );
}

#[test]
fn masterchain_snapshot_cache_expires_after_ttl() {
    assert!(masterchain_snapshot_cache_entry_is_fresh(
        100,
        100 + MASTERCHAIN_SNAPSHOT_CACHE_TTL.as_secs() - 1
    ));
    assert!(!masterchain_snapshot_cache_entry_is_fresh(
        100,
        100 + MASTERCHAIN_SNAPSHOT_CACHE_TTL.as_secs()
    ));
}

#[test]
fn masterchain_snapshot_cache_separates_builtin_and_custom_networks() {
    let root = Path::new("/tmp/acton-project/build/cache/masterchain-snapshots");
    let builtin = masterchain_snapshot_cache_path(root, &Network::Mainnet, 42);
    let custom = masterchain_snapshot_cache_path(root, &Network::Custom(Arc::from("mainnet")), 42);

    assert_ne!(builtin, custom);
    assert_eq!(builtin, root.join("mainnet").join("42.json"));
    assert_eq!(custom, root.join("custom-mainnet").join("42.json"));
}
