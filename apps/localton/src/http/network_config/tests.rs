//! Exercise the update workflow against a V2 server, including delayed activation.

use super::*;
use axum::{Router, extract::Path, routing::post};
use ed25519_dalek::Signature;
use expect_test::expect;

struct Chain {
    config: Dict<u32, Cell>,
    key: SigningKey,
    sent: usize,
    pending: Option<(u32, Cell)>,
    reads_after_send: usize,
}

async fn rpc(
    AxumState(chain): AxumState<Arc<Mutex<Chain>>>,
    Path(method): Path<String>,
    Json(payload): Json<Value>,
) -> Json<Value> {
    let mut chain = chain.lock().await;
    let result = match method.as_str() {
        "getConfigAll" => {
            if chain.sent > 0 {
                chain.reads_after_send += 1;
                if chain.reads_after_send >= 2
                    && let Some((index, value)) = chain.pending.take()
                {
                    chain.config.set(index, value).unwrap();
                }
            }
            json!({"config": {"bytes": STANDARD.encode(Boc::encode(chain.config.root().as_ref().unwrap()))}})
        }
        "getAddressInformation" => {
            let mut data = CellBuilder::new();
            data.store_reference(chain.config.root().clone().unwrap())
                .unwrap();
            data.store_u32(7).unwrap();
            data.store_raw(chain.key.verifying_key().as_bytes(), 256)
                .unwrap();
            data.store_bit_zero().unwrap();
            json!({"data": STANDARD.encode(Boc::encode(data.build().unwrap()))})
        }
        "sendBocReturnHash" => {
            let message =
                Boc::decode(STANDARD.decode(payload["boc"].as_str().unwrap()).unwrap()).unwrap();
            let body = message.as_slice().unwrap().load_reference_cloned().unwrap();
            let mut unsigned = body.as_slice().unwrap();
            let mut signature = [0_u8; 64];
            unsigned.load_raw(&mut signature, 512).unwrap();
            let mut signed = CellBuilder::new();
            signed.store_slice(unsigned).unwrap();
            chain
                .key
                .verifying_key()
                .verify_strict(
                    signed.build().unwrap().repr_hash().as_slice(),
                    &Signature::from_bytes(&signature),
                )
                .unwrap();
            assert_eq!(unsigned.load_u32().unwrap(), 0x4366_5021);
            assert_eq!(unsigned.load_u32().unwrap(), 7);
            unsigned.load_u32().unwrap();
            let index = unsigned.load_u32().unwrap();
            let value = unsigned.load_reference_cloned().unwrap();
            chain.pending = Some((index, value));
            chain.sent += 1;
            json!({"hash": "accepted"})
        }
        "getMasterchainInfo" => json!({"last": {"seqno": 123}}),
        _ => panic!("Unexpected V2 method {method}"),
    };
    Json(json!({"ok": true, "result": result}))
}

async fn fixture() -> (
    tempfile::TempDir,
    State,
    Arc<Mutex<Chain>>,
    tokio::task::JoinHandle<()>,
) {
    let temp = tempfile::tempdir().unwrap();
    let layout = Layout::new(temp.path().to_owned());
    std::fs::create_dir_all(&layout.zerostate).unwrap();
    let key = SigningKey::from_bytes(&[42; 32]);
    std::fs::write(layout.zerostate.join("config-master.pk"), key.to_bytes()).unwrap();
    let mut address = CellBuilder::new();
    address.store_raw(&[0x55; 32], 256).unwrap();
    let mut config = Dict::new();
    config.set(0_u32, address.build().unwrap()).unwrap();
    config
        .set(4_u32, CellBuilder::new().build().unwrap())
        .unwrap();
    let chain = Arc::new(Mutex::new(Chain {
        config,
        key,
        sent: 0,
        pending: None,
        reads_after_send: 0,
    }));
    let app = Router::new()
        .route("/api/v2/{method}", post(rpc))
        .with_state(chain.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    (temp, State::new(layout, endpoint), chain, task)
}

#[tokio::test]
async fn replaces_existing_and_adds_extension_only_after_masterchain_activation() {
    for index in [4, -12345] {
        let (_temp, state, chain, task) = fixture().await;
        let expected_hash = chain
            .lock()
            .await
            .config
            .get(index as u32)
            .unwrap()
            .map(|cell| cell.repr_hash().to_string());
        let mut value = CellBuilder::new();
        value.store_raw(&[0x42; 32], 256).unwrap();
        let cell = value.build().unwrap();
        let request = UpdateRequest {
            index,
            boc: STANDARD.encode(Boc::encode(&cell)),
            expected_hash,
        };
        let result = apply(&state, &request).await.unwrap();
        let chain = chain.lock().await;
        expect![["sent=1 reads=2 block=123 confirmed=true"]].assert_eq(&format!(
            "sent={} reads={} block={} confirmed={}",
            chain.sent,
            chain.reads_after_send,
            result.masterchain_seqno,
            result.hash == cell.repr_hash().to_string()
        ));
        task.abort();
    }
}

#[tokio::test]
async fn stale_and_invalid_updates_never_sign_or_send_a_message() {
    let (_temp, state, chain, task) = fixture().await;

    let current = chain.lock().await.config.get(4).unwrap().unwrap();
    let current_boc = STANDARD.encode(Boc::encode(&current));
    let stale = apply(
        &state,
        &UpdateRequest {
            index: 4,
            boc: current_boc.clone(),
            expected_hash: None,
        },
    )
    .await
    .err()
    .unwrap()
    .to_string();
    let unchanged = apply(
        &state,
        &UpdateRequest {
            index: 4,
            boc: current_boc,
            expected_hash: Some(current.repr_hash().to_string()),
        },
    )
    .await
    .err()
    .unwrap()
    .to_string();
    let immutable = apply(
        &state,
        &UpdateRequest {
            index: 0,
            boc: STANDARD.encode(Boc::encode(CellBuilder::new().build().unwrap())),
            expected_hash: None,
        },
    )
    .await
    .err()
    .unwrap()
    .to_string();
    let invalid = apply(
        &state,
        &UpdateRequest {
            index: 4,
            boc: "invalid".to_owned(),
            expected_hash: None,
        },
    )
    .await
    .err()
    .unwrap()
    .to_string();

    expect![[r#"
        stale=Parameter changed since it was loaded; reload the configuration before applying
        unchanged=The parameter has not changed
        immutable=Parameter 0 identifies this config contract and cannot be changed in place
        invalid=Parameter must be a base64 BoC
        sent=0
    "#]]
    .assert_eq(&format!(
        "stale={stale}\nunchanged={unchanged}\nimmutable={immutable}\ninvalid={invalid}\nsent={}\n",
        chain.lock().await.sent
    ));

    task.abort();
}
