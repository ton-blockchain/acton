use super::support::{Live, TypedResponse, fixture, invalid_boc};
use anyhow::{Context, Result};
use serde_json::json;
use ton_api::toncenter::v2;
use tycho_types::boc::Boc;
use tycho_types::cell::Cell;

const ELECTOR_ADDRESS: &str = "-1:3333333333333333333333333333333333333333333333333333333333333333";

fn live() -> Result<Option<Live>> {
    Live::from_env()
}

fn masterchain_info(live: &Live) -> Result<v2::MasterchainInfo> {
    let response: v2::TonlibResponse<v2::MasterchainInfo> =
        live.get(&live.v2_url, "/getMasterchainInfo", &v2::EmptyRequest {})?;
    Ok(response.result)
}

#[test]
#[ignore = "optional live TonCenter contract test"]
fn address_information_request_and_response_variants() -> Result<()> {
    let Some(live) = live()? else { return Ok(()) };
    let fixture = fixture(&live)?;
    let masterchain = masterchain_info(&live)?;

    for seqno in [None, Some(u32::try_from(masterchain.last.seqno)?)] {
        let _: v2::TonlibResponse<v2::AddressInformation> = live.get(
            &live.v2_url,
            "/getAddressInformation",
            &v2::AddressInformationRequest {
                address: fixture.transaction.account.clone(),
                seqno: seqno.map(Into::into),
            },
        )?;
    }
    Ok(())
}

#[test]
#[ignore = "optional live TonCenter contract test"]
fn address_request_detect_pack_and_unpack_responses() -> Result<()> {
    let Some(live) = live()? else { return Ok(()) };
    let fixture = fixture(&live)?;

    let detected: v2::TonlibResponse<v2::DetectAddress> = live.get(
        &live.v2_url,
        "/detectAddress",
        &v2::AddressRequest {
            address: fixture.transaction.account.clone(),
        },
    )?;
    let packed: v2::TonlibResponse<String> = live.get(
        &live.v2_url,
        "/packAddress",
        &v2::AddressRequest {
            address: detected.result.raw_form,
        },
    )?;
    let _: v2::TonlibResponse<String> = live.get(
        &live.v2_url,
        "/unpackAddress",
        &v2::AddressRequest {
            address: packed.result,
        },
    )?;
    Ok(())
}

#[test]
#[ignore = "optional live TonCenter contract test"]
fn detect_hash_request_accepts_base64_and_hex() -> Result<()> {
    let Some(live) = live()? else { return Ok(()) };
    let fixture = fixture(&live)?;

    let detected: v2::TonlibResponse<v2::DetectHash> = live.get(
        &live.v2_url,
        "/detectHash",
        &v2::DetectHashRequest {
            hash: fixture.transaction.hash.clone(),
        },
    )?;
    let _: v2::TonlibResponse<v2::DetectHash> = live.get(
        &live.v2_url,
        "/detectHash",
        &v2::DetectHashRequest {
            hash: detected.result.hex,
        },
    )?;
    Ok(())
}

#[test]
#[ignore = "optional live TonCenter contract test"]
fn libraries_request_accepts_one_and_multiple_hashes() -> Result<()> {
    let Some(live) = live()? else { return Ok(()) };
    let fixture = fixture(&live)?;

    for libraries in [
        vec![fixture.transaction.hash.clone()],
        vec![
            fixture.transaction.hash.clone(),
            "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=".to_owned(),
        ],
    ] {
        let _: v2::TonlibResponse<v2::LibraryResult> = live.get(
            &live.v2_url,
            "/getLibraries",
            &v2::LibrariesRequest { libraries },
        )?;
    }
    Ok(())
}

#[test]
#[ignore = "optional live TonCenter contract test"]
fn transactions_request_covers_limit_cursor_and_archival() -> Result<()> {
    let Some(live) = live()? else { return Ok(()) };
    let fixture = fixture(&live)?;

    let _: v2::TonlibResponse<Vec<v2::Transaction>> = live.get(
        &live.v2_url,
        "/getTransactions",
        &v2::TransactionsRequest {
            address: fixture.transaction.account.clone(),
            limit: Some(2.into()),
            lt: None,
            hash: None,
            to_lt: None,
            archival: Some(false),
        },
    )?;
    let _: v2::TonlibResponse<Vec<v2::Transaction>> = live.get(
        &live.v2_url,
        "/getTransactions",
        &v2::TransactionsRequest {
            address: fixture.transaction.account.clone(),
            limit: Some(2.into()),
            lt: Some(v2::StringOrNumber::String(fixture.transaction.lt.clone())),
            hash: Some(fixture.transaction.hash.clone()),
            to_lt: Some(0.into()),
            archival: Some(true),
        },
    )?;
    let _: v2::TonlibResponse<v2::RawTransactions> = live.get(
        &live.v2_url,
        "/getTransactionsStd",
        &v2::TransactionsRequest {
            address: fixture.transaction.account.clone(),
            limit: Some(2.into()),
            lt: None,
            hash: None,
            to_lt: None,
            archival: Some(false),
        },
    )?;
    Ok(())
}

#[test]
#[ignore = "optional live TonCenter contract test"]
fn block_transactions_ext_uses_raw_transaction_ext_wire_types() -> Result<()> {
    let Some(live) = live()? else { return Ok(()) };
    let masterchain = masterchain_info(&live)?;
    let shards: v2::TonlibResponse<v2::Shards> = live.get(
        &live.v2_url,
        "/getShards",
        &v2::SeqnoRequest {
            seqno: i32::try_from(masterchain.last.seqno)?.into(),
        },
    )?;
    let block = shards
        .result
        .shards
        .first()
        .context("latest masterchain block returned no shards")?;

    let response: v2::TonlibResponse<v2::BlockTransactionsExt> = live.get(
        &live.v2_url,
        "/getBlockTransactionsExt",
        &v2::BlockTransactionsRequest {
            workchain: block.workchain.into(),
            shard: v2::StringOrNumber::String(block.shard.clone()),
            seqno: i32::try_from(block.seqno)?.into(),
            root_hash: Some(block.root_hash.clone()),
            file_hash: Some(block.file_hash.clone()),
            after_lt: None,
            after_hash: None,
            count: Some(5.into()),
        },
    )?;

    let transaction = response
        .result
        .transactions
        .first()
        .context("fixture block returned no extended transactions")?;
    anyhow::ensure!(transaction.type_field == "raw.transactionExt");
    if let Some(message) = transaction.in_msg.as_ref() {
        anyhow::ensure!(message.type_field == "raw.message");
        anyhow::ensure!(message.source.type_field == "accountAddress");
        anyhow::ensure!(message.destination.type_field == "accountAddress");
    }
    Ok(())
}

#[test]
#[ignore = "optional live TonCenter contract test"]
fn try_locate_tx_request_and_transaction_response() -> Result<()> {
    let Some(live) = live()? else { return Ok(()) };
    let fixture = fixture(&live)?;
    let message = fixture
        .transaction
        .in_msg
        .iter()
        .chain(fixture.transaction.out_msgs.iter())
        .find(|message| {
            message.source.is_some()
                && message.destination.is_some()
                && message.created_lt.is_some()
        });
    let Some(message) = message else {
        return Ok(());
    };

    let _: v2::TonlibResponse<v2::Transaction> = live.get(
        &live.v2_url,
        "/tryLocateTx",
        &v2::TryLocateTxRequest {
            source: message.source.clone().context("source disappeared")?,
            destination: message
                .destination
                .clone()
                .context("destination disappeared")?,
            created_lt: v2::StringOrNumber::String(
                message
                    .created_lt
                    .clone()
                    .context("created_lt disappeared")?,
            ),
        },
    )?;
    Ok(())
}

#[test]
#[ignore = "optional live TonCenter contract test"]
fn config_param_request_covers_param_alias_and_seqno() -> Result<()> {
    let Some(live) = live()? else { return Ok(()) };
    let seqno = i32::try_from(masterchain_info(&live)?.last.seqno)?;

    for request in [
        v2::ConfigParamRequest {
            param: Some(0.into()),
            config_id: None,
            seqno: None,
        },
        v2::ConfigParamRequest {
            param: None,
            config_id: Some(0.into()),
            seqno: Some(seqno.into()),
        },
    ] {
        let _: v2::TonlibResponse<v2::ConfigInfo> =
            live.get(&live.v2_url, "/getConfigParam", &request)?;
    }
    Ok(())
}

#[test]
#[ignore = "optional live TonCenter contract test"]
fn config_all_request_covers_latest_and_explicit_seqno() -> Result<()> {
    let Some(live) = live()? else { return Ok(()) };
    let seqno = i32::try_from(masterchain_info(&live)?.last.seqno)?;

    for seqno in [None, Some(seqno.into())] {
        let _: v2::TonlibResponse<v2::ConfigInfo> = live.get(
            &live.v2_url,
            "/getConfigAll",
            &v2::ConfigAllRequest { seqno },
        )?;
    }
    Ok(())
}

#[test]
#[ignore = "optional live TonCenter contract test"]
fn block_header_request_covers_id_and_hashes() -> Result<()> {
    let Some(live) = live()? else { return Ok(()) };
    let block = masterchain_info(&live)?.last;

    for include_hashes in [false, true] {
        let _: v2::TonlibResponse<v2::BlockHeader> = live.get(
            &live.v2_url,
            "/getBlockHeader",
            &v2::BlockHeaderRequest {
                workchain: block.workchain.into(),
                shard: v2::StringOrNumber::String(block.shard.clone()),
                seqno: i32::try_from(block.seqno)?.into(),
                root_hash: include_hashes.then(|| block.root_hash.clone()),
                file_hash: include_hashes.then(|| block.file_hash.clone()),
            },
        )?;
    }
    Ok(())
}

#[test]
#[ignore = "optional live TonCenter contract test"]
fn lookup_block_request_covers_seqno_lt_and_unixtime() -> Result<()> {
    let Some(live) = live()? else { return Ok(()) };
    let block = &fixture(&live)?.block;
    let gen_utime = v2::StringOrNumber::String(block.gen_utime.to_bigint()?.to_string());

    for request in [
        v2::LookupBlockRequest {
            workchain: block.workchain.into(),
            shard: v2::StringOrNumber::String(block.shard.clone()),
            seqno: Some(i32::try_from(block.seqno)?.into()),
            lt: None,
            unixtime: None,
        },
        v2::LookupBlockRequest {
            workchain: block.workchain.into(),
            shard: v2::StringOrNumber::String(block.shard.clone()),
            seqno: None,
            lt: Some(v2::StringOrNumber::String(block.start_lt.clone())),
            unixtime: None,
        },
        v2::LookupBlockRequest {
            workchain: block.workchain.into(),
            shard: v2::StringOrNumber::String(block.shard.clone()),
            seqno: None,
            lt: None,
            unixtime: Some(gen_utime),
        },
    ] {
        let _: v2::TonlibResponse<v2::TonBlockIdExt> =
            live.get(&live.v2_url, "/lookupBlock", &request)?;
    }
    Ok(())
}

#[test]
#[ignore = "optional live TonCenter contract test"]
fn run_get_method_request_covers_latest_and_historical_state() -> Result<()> {
    let Some(live) = live()? else { return Ok(()) };
    let seqno = u32::try_from(masterchain_info(&live)?.last.seqno)?;

    for seqno in [None, Some(seqno)] {
        let _: v2::TonlibResponse<v2::RunGetMethodResult> = live.post(
            &live.v2_url,
            "/runGetMethod",
            &v2::RunGetMethodRequest {
                address: ELECTOR_ADDRESS.to_owned(),
                method: v2::StringOrNumber::String("participant_list_extended".to_owned()),
                stack: Vec::new(),
                seqno: seqno.map(Into::into),
            },
        )?;

        let _: v2::TonlibResponse<v2::RunGetMethodStdResult> = live.post(
            &live.v2_url,
            "/runGetMethodStd",
            &v2::RunGetMethodStdRequest {
                address: ELECTOR_ADDRESS.to_owned(),
                method: v2::StringOrNumber::String("participant_list_extended".to_owned()),
                stack: Vec::new(),
                seqno: seqno.map(|value| value as i32),
            },
        )?;
    }

    let boc = Boc::encode_base64(Cell::default());
    let _: v2::TonlibResponse<v2::RunGetMethodStdResult> = live.post(
        &live.v2_url,
        "/runGetMethodStd",
        &v2::RunGetMethodStdRequest {
            address: ELECTOR_ADDRESS.to_owned(),
            method: v2::StringOrNumber::Number(1),
            stack: vec![
                v2::TvmStackEntry::number(7),
                v2::TvmStackEntry::cell(boc.clone()),
                v2::TvmStackEntry::slice(boc),
                v2::TvmStackEntry::tuple(Vec::new()),
                v2::TvmStackEntry::list(Vec::new()),
            ],
            seqno: None,
        },
    )?;
    Ok(())
}

#[test]
#[ignore = "optional live TonCenter contract test"]
fn json_rpc_request_and_generic_response() -> Result<()> {
    let Some(live) = live()? else { return Ok(()) };

    for request in [
        json!({"method": "getMasterchainInfo"}),
        json!({"method": "getMasterchainInfo", "params": []}),
        json!({"method": "getMasterchainInfo", "params": null}),
        json!({
            "jsonrpc": 2,
            "id": {"ignored": true},
            "method": "getMasterchainInfo",
            "params": "ignored"
        }),
    ] {
        let response: v2::JsonRpcResponse<v2::MasterchainInfo> =
            live.post(&live.v2_url, "/jsonRPC", &request)?;
        assert!(response.jsonrpc.is_none());
        assert!(response.id.is_none());
    }

    let _: v2::TonlibErrorResponse = live.post_error(
        &live.v2_url,
        "/jsonRPC",
        &json!({"method": "getMasterchainInfo", "params": [{}]}),
    )?;

    let seqno = masterchain_info(&live)?.last.seqno;
    let _: v2::JsonRpcResponse<v2::Shards> = live.post(
        &live.v2_url,
        "/jsonRPC",
        &json!({"method": "getShards", "params": {"seqno": seqno}}),
    )?;
    Ok(())
}

#[test]
#[ignore = "optional live TonCenter contract test"]
fn send_boc_request_deserializes_real_error_without_broadcasting() -> Result<()> {
    let Some(live) = live()? else { return Ok(()) };

    let response: TypedResponse<v2::TonlibResponse<v2::ResultOk>, v2::TonlibErrorResponse> = live
        .post_either(
        &live.v2_url,
        "/sendBoc",
        &v2::SendBocRequest {
            boc: invalid_boc().to_owned(),
        },
    )?;
    match response {
        TypedResponse::Success(response) => {
            anyhow::bail!(
                "invalid BOC unexpectedly accepted: {}",
                response.result.type_field
            )
        }
        TypedResponse::Error(_) => {}
    }

    let response: TypedResponse<v2::TonlibResponse<v2::ExtMessageInfo>, v2::TonlibErrorResponse> =
        live.post_either(
            &live.v2_url,
            "/sendBocReturnHash",
            &v2::SendBocRequest {
                boc: invalid_boc().to_owned(),
            },
        )?;
    if let TypedResponse::Success(response) = response {
        anyhow::bail!(
            "invalid BOC unexpectedly accepted: {}",
            response.result.hash
        );
    }
    Ok(())
}

#[test]
#[ignore = "optional live TonCenter contract test"]
fn json_rpc_typed_params_cover_address_information() -> Result<()> {
    let Some(live) = live()? else { return Ok(()) };
    let fixture = fixture(&live)?;

    let _: v2::JsonRpcResponse<v2::AddressInformation> = live.post(
        &live.v2_url,
        "/jsonRPC",
        &v2::JsonRpcRequest::new(
            "live-address-information",
            "getAddressInformation",
            v2::AddressInformationRequest {
                address: fixture.transaction.account.clone(),
                seqno: None,
            },
        ),
    )?;

    Ok(())
}
