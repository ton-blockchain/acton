use anyhow::Result;
use expect_test::expect_file;
use rston::cell::{Cell, CellBuilder, CellFamily};
use rston::models::{
    AccountState, CurrencyCollection, ExtInMsgInfo, IntAddr, LibDescr, OwnedMessage,
    OwnedRelaxedMessage, RelaxedExtOutMsgInfo, RelaxedIntMsgInfo, RelaxedMsgInfo, StateInit,
    StdAddr,
};
use serde_json::{Value, json};

use super::*;
use crate::api::tests::fixture::{Fixture, NATIVE};

fn address() -> StdAddr {
    "0:6e3eecb46e7a3003672e9032b0fff6155f4e02b14a57a6ba73337f428053d399"
        .parse()
        .unwrap()
}

fn active(code: Cell) -> Result<AccountState> {
    Ok(AccountState::Active(StateInit {
        code: Some(code),
        data: Some(CellBuilder::build_from(7_u32)?),
        ..Default::default()
    }))
}

fn external(body: Cell) -> Result<Cell> {
    Ok(CellBuilder::build_from(OwnedMessage {
        info: MsgInfo::ExtIn(ExtInMsgInfo {
            dst: IntAddr::Std(address()),
            ..Default::default()
        }),
        init: None,
        body: body.into(),
        layout: None,
    })?)
}

fn transfer(destination: StdAddr, bounce: bool) -> Result<Cell> {
    Ok(CellBuilder::build_from(OwnedRelaxedMessage {
        info: RelaxedMsgInfo::Int(RelaxedIntMsgInfo {
            dst: IntAddr::Std(destination),
            value: CurrencyCollection::new(100_000_000),
            bounce,
            ihr_disabled: true,
            ..Default::default()
        }),
        init: None,
        body: Cell::empty_cell().into(),
        layout: None,
    })?)
}

fn with_messages(messages: Vec<Cell>) -> Result<Cell> {
    let mut builder = CellBuilder::new();
    for message in messages {
        builder.store_reference(message)?;
    }
    external(builder.build()?)
}

async fn request(fixture: &Fixture, body: Value) -> Result<Value> {
    fixture
        .raw_at("/api/simulate", &serde_json::to_vec(&body)?)
        .await
}

fn compact(response: &Value) -> Value {
    if response["status"] != 200 {
        return response.clone();
    }
    let body = &response["body"];
    let transactions = body["transactions"].as_object().unwrap();
    let mut rows: Vec<_> = transactions.values().collect();
    rows.sort_by_key(|tx| tx["lt"].as_str().unwrap().parse::<u64>().unwrap());
    json!({
        "status": response["status"],
        "mc_block_seqno": body["mc_block_seqno"],
        "is_incomplete": body["is_incomplete"],
        "transactions": rows.iter().map(|tx| json!({
            "account": tx["account"], "lt": tx["lt"], "now": tx["now"],
            "aborted": tx["description"]["aborted"],
            "compute": tx["description"]["compute_ph"],
            "bounce": tx["description"]["bounce"],
            "before": tx["account_state_before"], "after": tx["account_state_after"],
            "children": tx["child_transactions"].as_array().unwrap().len(),
        })).collect::<Vec<_>>()
    })
}

#[tokio::test]
async fn simulates_native_traces_without_changing_the_checkpoint() -> Result<()> {
    let _native = NATIVE.lock().await;
    let contracts: Value = serde_json::from_str(include_str!("fixtures/contracts.json"))?;
    let counter = Boc::decode_base64(contracts["counter"].as_str().unwrap())?;
    let fixture = Fixture::new(active(counter.clone())?, Dict::new())?;
    let before = fixture.store.get_account(&address())?.account.unwrap();
    let receiver = StdAddr::new(0, HashBytes([2; 32]));
    let external_output = CellBuilder::build_from(OwnedRelaxedMessage {
        info: RelaxedMsgInfo::ExtOut(RelaxedExtOutMsgInfo {
            src: Some(IntAddr::Std(address())),
            ..Default::default()
        }),
        init: None,
        body: CellBuilder::build_from(123_u32)?.into(),
        layout: None,
    })?;
    let message = with_messages(vec![
        transfer(receiver.clone(), false)?,
        transfer(receiver.clone(), false)?,
        transfer(StdAddr::new(0, HashBytes([3; 32])), true)?,
        external_output,
    ])?;
    let input = json!({"boc": Boc::encode_base64(&message)});
    let response = request(&fixture, input.clone()).await?;
    expect_file!["snapshots/trace.json"].assert_eq(&serde_json::to_string_pretty(&response)?);

    let mut with_cells = input.clone();
    with_cells["include_code_data"] = json!(true);
    let cells = request(&fixture, with_cells).await?;
    let repeated = request(&fixture, input.clone()).await?;
    let after = fixture.store.get_account(&address())?.account.unwrap();
    let body = &cells["body"];
    let code_cells = body["code_cells"].as_object().context("code map absent")?;
    let data_cells = body["data_cells"].as_object().context("data map absent")?;
    let mut counters: Vec<u32> = data_cells
        .values()
        .map(|boc| -> Result<_> {
            Ok(Boc::decode_base64(boc.as_str().unwrap())?
                .as_slice()?
                .load_u32()?)
        })
        .collect::<Result<_>>()?;
    counters.sort_unstable();
    expect_file!["snapshots/isolation.json"].assert_eq(&serde_json::to_string_pretty(&json!({
        "same_response": repeated == response,
        "same_transactions_with_cells": body["transactions"] == response["body"]["transactions"],
        "stored_state_unchanged": before.account.inner().repr_hash() == after.account.inner().repr_hash(),
        "receiver_not_written": fixture.store.get_account(&receiver)?.account.is_none(),
        "code_cells": code_cells.len(), "counter_values": counters,
        "cell_hashes_valid": code_cells.iter().chain(data_cells.iter()).all(|(hash,boc)|
            STANDARD.encode(Boc::decode_base64(boc.as_str().unwrap()).unwrap().repr_hash()) == *hash),
    }))?);

    let AccountState::Active(init) = active(counter.clone())? else {
        unreachable!()
    };
    let deployed_address = StdAddr::new(0, *CellBuilder::build_from(&init)?.repr_hash());
    let mut deploy = transfer(deployed_address.clone(), false)?.parse::<OwnedRelaxedMessage>()?;
    deploy.init = Some(init);
    let response = request(
        &fixture,
        json!({
            "boc": Boc::encode_base64(with_messages(vec![CellBuilder::build_from(deploy)?])?),
            "include_code_data": true,
        }),
    )
    .await?;
    expect_file!["snapshots/deployment.json"].assert_eq(&serde_json::to_string_pretty(&json!({
        "response": compact(&response),
        "not_persisted": fixture.store.get_account(&deployed_address)?.account.is_none(),
    }))?);

    let mut errors = Vec::new();
    for (key, value) in [
        ("boc", json!("?")),
        ("boc", json!("")),
        (
            "boc",
            json!(Boc::encode_base64(transfer(address(), false)?)),
        ),
        ("boc", json!(STANDARD.encode(vec![0; 65_536]))),
        ("with_actions", json!(true)),
        ("include_metadata", json!(true)),
        ("include_address_book", json!(true)),
        ("unknown", json!(true)),
        ("mc_block_seqno", json!(99)),
        ("mc_block_seqno", json!(-1)),
    ] {
        let mut body = input.clone();
        body[key] = value;
        errors.push(request(&fixture, body).await?);
    }
    errors.push(fixture.raw_at("/api/simulate", b"{").await?);
    errors.push(
        fixture
            .raw_at("/api/simulate", &vec![b' '; 2 * 1024 * 1024 + 1])
            .await?,
    );
    let permit = fixture.api.execution_slot.acquire().await?;
    errors.push(request(&fixture, input.clone()).await?);
    errors.push(
        fixture
            .request(json!({"address":address().to_string(), "method":"seqno", "stack":[]}))
            .await?,
    );
    drop(permit);
    expect_file!["snapshots/errors.json"].assert_eq(&serde_json::to_string_pretty(&errors)?);

    let mut executions = Vec::new();
    for name in ["reject", "abort", "relay", "fork"] {
        let fixture = Fixture::new(
            active(Boc::decode_base64(contracts[name].as_str().unwrap())?)?,
            Dict::new(),
        )?;
        let message = with_messages(vec![transfer(address(), false)?])?;
        let response = request(&fixture, json!({"boc":Boc::encode_base64(message)})).await?;
        executions.push(json!({"contract":name,"response":if ["relay","fork"].contains(&name) && response["status"] == 200 {
            json!({"status":response["status"],"transactions":response["body"]["transactions"].as_object().unwrap().len(),"is_incomplete":response["body"]["is_incomplete"]})
        } else { compact(&response) }}));
    }
    expect_file!["snapshots/execution.json"].assert_eq(&serde_json::to_string_pretty(&executions)?);

    let mut reference = CellBuilder::new();
    reference.set_exotic(true);
    reference.store_u8(2)?;
    reference.store_u256(counter.repr_hash())?;
    let reference = reference.build()?;
    let mut libraries = Dict::new();
    let mut publishers = Dict::new();
    publishers.set(address().address, ())?;
    libraries.set(
        *counter.repr_hash(),
        LibDescr {
            lib: counter,
            publishers,
        },
    )?;
    let mut rows = Vec::new();
    for libraries in [Dict::new(), libraries] {
        let fixture = Fixture::new(active(reference.clone())?, libraries)?;
        rows.push(compact(
            &request(
                &fixture,
                json!({"boc":Boc::encode_base64(external(Cell::empty_cell())?)}),
            )
            .await?,
        ));
    }
    expect_file!["snapshots/libraries.json"].assert_eq(&serde_json::to_string_pretty(&rows)?);

    let wallet: Value =
        serde_json::from_str(include_str!("../get_method/fixtures/wallet-v5.json"))?;
    let data = Boc::decode_base64(wallet["account"]["data"].as_str().unwrap())?;
    let fixture = Fixture::new(
        AccountState::Active(StateInit {
            code: Some(Boc::decode_base64(
                wallet["account"]["code"].as_str().unwrap(),
            )?),
            data: Some(data.clone()),
            ..Default::default()
        }),
        Dict::new(),
    )?;
    let mut storage = data.as_slice()?;
    storage.load_bit()?;
    let seqno = storage.load_u32()?;
    let wallet_id = storage.load_u32()?;
    let mut rows = Vec::new();
    for (ignore, valid_until, request_seqno) in [
        (false, u32::MAX, seqno),
        (true, u32::MAX, seqno),
        (true, 1, seqno),
        (true, u32::MAX, seqno + 1),
    ] {
        let mut body = CellBuilder::new();
        body.store_u32(0x7369_676e)?;
        body.store_u32(wallet_id)?;
        body.store_u32(valid_until)?;
        body.store_u32(request_seqno)?;
        body.store_bit_zero()?; // No out-actions list
        body.store_bit_zero()?; // No extended actions
        body.store_raw(&[0; 64], 512)?; // Intentionally invalid signature
        let response = request(
            &fixture,
            json!({
                "boc":Boc::encode_base64(external(body.build()?)?),
                "ignore_chksig":ignore, "include_code_data":true, "mc_block_seqno":100,
            }),
        )
        .await?;
        let updated_seqno = response["body"]["transactions"]
            .as_object()
            .and_then(|txs| txs.values().next())
            .map(|tx| -> Result<_> {
                let hash = tx["account_state_after"]["data_hash"].as_str().unwrap();
                let cell =
                    Boc::decode_base64(response["body"]["data_cells"][hash].as_str().unwrap())?;
                let mut slice = cell.as_slice()?;
                slice.load_bit()?;
                Ok(slice.load_u32()?)
            })
            .transpose()?;
        rows.push(
            json!({"ignore_chksig":ignore, "valid_until":valid_until, "seqno":request_seqno,
            "updated_seqno":updated_seqno, "response":compact(&response)}),
        );
    }
    expect_file!["snapshots/wallet-v5.json"].assert_eq(&serde_json::to_string_pretty(&rows)?);
    Ok(())
}
