use expect_test::expect_file;
use rston::cell::CellBuilder;
use rston::models::{LibDescr, StateInit};
use serde_json::{Value, json};

use super::*;
use crate::api::tests::fixture::{Fixture, NATIVE};

fn active(code: Cell, data: Cell) -> AccountState {
    AccountState::Active(StateInit {
        code: Some(code),
        data: Some(data),
        ..Default::default()
    })
}

fn result(response: &Value) -> Value {
    let body = &response["body"];
    if let Some(result) = body.get("result") {
        json!({ "status": response["status"], "ok": body["ok"], "gas_used": result["gas_used"], "exit_code": result["exit_code"], "stack": result["stack"], "mc_seqno": result["block_id"]["seqno"], "lt": result["last_transaction_id"]["lt"] })
    } else {
        response.clone()
    }
}

#[tokio::test]
async fn run_get_method_matches_toncenter_and_preserves_the_committed_state() -> Result<()> {
    let _native = NATIVE.lock().await;
    let wallet: Value = serde_json::from_str(include_str!("fixtures/wallet-v5.json"))?;
    let code = Boc::decode_base64(wallet["account"]["code"].as_str().unwrap())?;
    let data = Boc::decode_base64(wallet["account"]["data"].as_str().unwrap())?;
    let fixture = Fixture::new(active(code, data.clone()), Dict::new())?;
    let base = json!({ "address": wallet["address"], "method": "seqno", "stack": [] });
    let mut rows = Vec::new();
    for method in [
        json!("seqno"),
        json!(85143),
        json!("0x14c97"),
        json!("get_extensions"),
        json!("missing_method"),
    ] {
        let mut request = base.clone();
        request["method"] = method.clone();
        let response = fixture.request(request).await?;
        let compact = result(&response);
        let oracle = match method.as_str() {
            Some("get_extensions") => &wallet["get_extensions"],
            Some("missing_method") => &wallet["missing_method"],
            _ => &wallet["seqno"],
        };
        rows.push(json!({
            "method": method,
            "response": compact,
            "matches_toncenter": compact["gas_used"] == oracle["gas_used"]
                && compact["exit_code"] == oracle["exit_code"]
                && compact["stack"] == oracle["stack"],
        }));
    }
    expect_file!["snapshots/wallet.json"].assert_eq(&serde_json::to_string_pretty(&rows)?);

    let contracts: Value = serde_json::from_str(include_str!("fixtures/contracts.json"))?;
    let mut rows = Vec::new();
    for name in [
        "identity",
        "context",
        "out_of_gas",
        "throw",
        "write_data",
        "alternative_return",
        "nan",
        "slice_offset",
        "continuation",
        "builder",
        "deep_result",
        "shared_tuple_result",
    ] {
        let code = Boc::decode_base64(contracts[name].as_str().unwrap())?;
        let fixture = Fixture::new(active(code, data.clone()), Dict::new())?;
        let address: StdAddr = wallet["address"].as_str().unwrap().parse()?;
        let before = fixture
            .store
            .get_account(&address)?
            .account
            .unwrap()
            .account
            .inner()
            .repr_hash()
            .to_string();
        let mut request = base.clone();
        if name == "slice_offset" {
            request["stack"] = json!([[
                "tvm.Slice",
                Boc::encode_base64(CellBuilder::build_from(0x1234_u16)?)
            ]]);
        }
        rows.push(json!({ "contract": name, "response": result(&fixture.request(request).await?),
            "state_unchanged": before == fixture.store.get_account(&address)?.account.unwrap().account.inner().repr_hash().to_string() }));
    }
    expect_file!["snapshots/execution.json"].assert_eq(&serde_json::to_string_pretty(&rows)?);

    let echo = Fixture::new(
        active(
            Boc::decode_base64(contracts["identity"].as_str().unwrap())?,
            data.clone(),
        ),
        Dict::new(),
    )?;
    let cell = Boc::encode_base64(CellBuilder::build_from(0x1234_u16)?);
    let maximum = (num_bigint::BigInt::from(1) << 256_usize) - 1_u32;
    let minimum = -(num_bigint::BigInt::from(1) << 256_usize);
    let typed_number = json!({ "@type": "tvm.stackEntryNumber", "number": { "@type": "tvm.numberDecimal", "number": "123" } });
    let mut rows = Vec::new();
    for (name, input) in [
        (
            "integer_aliases",
            json!([
                ["int", -1],
                ["integer", "-0x2"],
                ["number", "123"],
                ["num", maximum.to_string()],
                ["num", minimum.to_string()]
            ]),
        ),
        (
            "cell_and_slice",
            json!([["tvm.Cell", cell], ["cell", {"bytes": cell}], ["tvm.Slice", cell], ["slice", {"bytes": cell}]]),
        ),
        (
            "empty_tuple",
            json!([["tuple", {"@type": "tvm.tuple", "elements": []}]]),
        ),
        (
            "empty_list",
            json!([["list", {"@type": "tvm.list", "elements": []}]]),
        ),
        (
            "list",
            json!([["tvm.List", {"@type": "tvm.list", "elements": [typed_number, typed_number]}]]),
        ),
        (
            "nested",
            json!([["tvm.Tuple", {"@type": "tvm.tuple", "elements": [typed_number, {"@type":"tvm.stackEntryTuple", "tuple": {"@type":"tvm.tuple", "elements":[typed_number]}}]}]]),
        ),
        (
            "int_overflow",
            json!([["num", (-minimum.clone()).to_string()]]),
        ),
        ("invalid_sign", json!([["num", "--1"]])),
        ("too_many_arguments", json!(vec![json!(["num", 1]); 257])),
        (
            "deep_list",
            json!([["list", {"@type":"tvm.list", "elements":vec![typed_number.clone(); 65]}]]),
        ),
        (
            "oversized_boc",
            json!([["tvm.Cell", "A".repeat(1024 * 1024 + 1)]]),
        ),
    ] {
        let mut request = base.clone();
        request["stack"] = input;
        rows.push(json!({ "case": name, "response": result(&echo.request(request).await?) }));
    }
    expect_file!["snapshots/stack.json"].assert_eq(&serde_json::to_string_pretty(&rows)?);

    let identity = Boc::decode_base64(contracts["identity"].as_str().unwrap())?;
    let mut library_reference = CellBuilder::new();
    library_reference.set_exotic(true);
    library_reference.store_u8(2)?;
    library_reference.store_u256(identity.repr_hash())?;
    let library_reference = library_reference.build()?;
    let mut libraries = Dict::new();
    let mut publishers = Dict::new();
    publishers.set(HashBytes([1; 32]), ())?;
    libraries.set(
        *identity.repr_hash(),
        LibDescr {
            lib: identity,
            publishers,
        },
    )?;
    let mut rows = Vec::new();
    for libraries in [Dict::new(), libraries] {
        let fixture = Fixture::new(active(library_reference.clone(), data.clone()), libraries)?;
        rows.push(result(&fixture.request(base.clone()).await?));
    }
    for state in [
        AccountState::Uninit,
        AccountState::Frozen(HashBytes([8; 32])),
        AccountState::Active(StateInit::default()),
    ] {
        let fixture = Fixture::new(state, Dict::new())?;
        let mut request = base.clone();
        request["stack"] = json!([["num", 5]]);
        rows.push(result(&fixture.request(request.clone()).await?));
        request["address"] = json!(StdAddr::new(0, HashBytes([2; 32])).to_string());
        rows.push(result(&fixture.request(request).await?));
    }
    expect_file!["snapshots/lifecycle.json"].assert_eq(&serde_json::to_string_pretty(&rows)?);

    let mut rows = Vec::new();
    for (key, value) in [
        ("address", json!("invalid")),
        ("seqno", json!(-1)),
        ("seqno", json!(99)),
        ("seqno", json!(100)),
        ("method", json!("")),
        ("method", json!("4294967296")),
        ("stack", json!([["num", "broken"]])),
        ("stack", json!([["cell", {"bytes": "broken"}]])),
        ("stack", json!([["unsupported", ""]])),
        ("extra", json!(true)),
    ] {
        let mut request = base.clone();
        request[key] = value.clone();
        rows.push(
            json!({ "input": [key, value], "response": result(&fixture.request(request).await?) }),
        );
    }
    rows.push(fixture.raw(b"{").await?);
    rows.push(fixture.raw(&vec![b' '; 2 * 1024 * 1024 + 1]).await?);
    let permit = fixture.api.execution_slot.acquire().await?;
    rows.push(fixture.request(base).await?);
    drop(permit);
    expect_file!["snapshots/errors.json"].assert_eq(&serde_json::to_string_pretty(&rows)?);
    Ok(())
}
