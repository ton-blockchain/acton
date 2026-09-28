use super::support::{Live, TypedResponse, fixture, invalid_boc};
use anyhow::{Context, Result};
use rston::boc::Boc;
use rston::cell::Cell;
use serde_json::json;
use toncenter::v2;

const ELECTOR_ADDRESS: &str = "-1:3333333333333333333333333333333333333333333333333333333333333333";

fn live() -> Result<Option<Live>> {
    Live::from_env()
}

fn masterchain_info(live: &Live) -> Result<v2::responses::MasterchainInfo> {
    let response: v2::TonlibResponse<v2::responses::MasterchainInfo> = live.get(
        &live.v2_url,
        "/getMasterchainInfo",
        &v2::requests::EmptyRequest {},
    )?;
    Ok(response.result)
}

#[test]
#[ignore = "optional live TON Center contract test"]
fn address_information_request_and_response_variants() -> Result<()> {
    let Some(live) = live()? else { return Ok(()) };
    let fixture = fixture(&live)?;
    let masterchain = masterchain_info(&live)?;

    for seqno in [None, Some(i32::try_from(masterchain.last.seqno)?)] {
        let _: v2::TonlibResponse<v2::responses::AddressInformation> = live.get(
            &live.v2_url,
            "/getAddressInformation",
            &v2::requests::AddressInformationRequest {
                address: fixture.transaction.account.clone(),
                seqno: seqno.map(Into::into),
            },
        )?;
    }
    Ok(())
}

#[test]
#[ignore = "optional live TON Center contract test"]
fn address_request_detect_pack_and_unpack_responses() -> Result<()> {
    let Some(live) = live()? else { return Ok(()) };
    let fixture = fixture(&live)?;

    let detected: v2::TonlibResponse<v2::responses::DetectAddress> = live.get(
        &live.v2_url,
        "/detectAddress",
        &v2::requests::AddressRequest {
            address: fixture.transaction.account.clone(),
        },
    )?;
    let packed: v2::TonlibResponse<String> = live.get(
        &live.v2_url,
        "/packAddress",
        &v2::requests::AddressRequest {
            address: detected.result.raw_form,
        },
    )?;
    let _: v2::TonlibResponse<String> = live.get(
        &live.v2_url,
        "/unpackAddress",
        &v2::requests::AddressRequest {
            address: packed.result,
        },
    )?;
    Ok(())
}

#[test]
#[ignore = "optional live TON Center contract test"]
fn detect_hash_request_accepts_base64_and_hex() -> Result<()> {
    let Some(live) = live()? else { return Ok(()) };
    let fixture = fixture(&live)?;

    let detected: v2::TonlibResponse<v2::responses::DetectHash> = live.get(
        &live.v2_url,
        "/detectHash",
        &v2::requests::DetectHashRequest {
            hash: fixture.transaction.hash.clone(),
        },
    )?;
    let _: v2::TonlibResponse<v2::responses::DetectHash> = live.get(
        &live.v2_url,
        "/detectHash",
        &v2::requests::DetectHashRequest {
            hash: detected.result.hex,
        },
    )?;
    Ok(())
}

#[test]
#[ignore = "optional live TON Center contract test"]
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
        let _: v2::TonlibResponse<v2::responses::LibraryResult> = live.get(
            &live.v2_url,
            "/getLibraries",
            &v2::requests::LibrariesRequest {
                libraries: Some(libraries),
            },
        )?;
    }
    Ok(())
}

#[test]
#[ignore = "optional live TON Center contract test"]
fn transactions_request_covers_limit_cursor_and_archival() -> Result<()> {
    let Some(live) = live()? else { return Ok(()) };
    let fixture = fixture(&live)?;

    let _: v2::TonlibResponse<Vec<v2::responses::Transaction>> = live.get(
        &live.v2_url,
        "/getTransactions",
        &v2::requests::TransactionsRequest {
            address: fixture.transaction.account.clone(),
            limit: Some(2.into()),
            lt: None,
            hash: None,
            to_lt: None,
            archival: Some(false.into()),
        },
    )?;
    let _: v2::TonlibResponse<Vec<v2::responses::Transaction>> = live.get(
        &live.v2_url,
        "/getTransactions",
        &v2::requests::TransactionsRequest {
            address: fixture.transaction.account.clone(),
            limit: Some(2.into()),
            lt: Some(fixture.transaction.lt.clone().into()),
            hash: Some(fixture.transaction.hash.clone()),
            to_lt: Some(0.into()),
            archival: Some(true.into()),
        },
    )?;
    let _: v2::TonlibResponse<v2::responses::TransactionsStd> = live.get(
        &live.v2_url,
        "/getTransactionsStd",
        &v2::requests::TransactionsRequest {
            address: fixture.transaction.account.clone(),
            limit: Some(2.into()),
            lt: None,
            hash: None,
            to_lt: None,
            archival: Some(false.into()),
        },
    )?;
    Ok(())
}

#[test]
#[ignore = "optional live TON Center contract test"]
fn block_transactions_ext_uses_raw_transaction_ext_wire_types() -> Result<()> {
    let Some(live) = live()? else { return Ok(()) };
    let masterchain = masterchain_info(&live)?;
    let shards: v2::TonlibResponse<v2::responses::Shards> = live.get(
        &live.v2_url,
        "/getShards",
        &v2::requests::SeqnoRequest {
            seqno: i32::try_from(masterchain.last.seqno)?.into(),
        },
    )?;
    let block = shards
        .result
        .shards
        .first()
        .context("latest masterchain block returned no shards")?;

    let response: v2::TonlibResponse<v2::responses::BlockTransactionsExt> = live.get(
        &live.v2_url,
        "/getBlockTransactionsExt",
        &v2::requests::BlockTransactionsRequest {
            workchain: i32::try_from(block.workchain)?.into(),
            shard: block.shard.clone().into(),
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
    anyhow::ensure!(transaction.type_tag == Default::default());
    if let Some(message) = transaction.in_msg.as_ref() {
        anyhow::ensure!(message.type_tag == Default::default());
        anyhow::ensure!(message.source.type_tag == Default::default());
        anyhow::ensure!(message.destination.type_tag == Default::default());
    }
    Ok(())
}

#[test]
#[ignore = "optional live TON Center contract test"]
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

    let _: v2::TonlibResponse<v2::responses::Transaction> = live.get(
        &live.v2_url,
        "/tryLocateTx",
        &v2::requests::TryLocateTxRequest {
            source: message.source.clone().context("source disappeared")?,
            destination: message
                .destination
                .clone()
                .context("destination disappeared")?,
            created_lt: (message
                .created_lt
                .clone()
                .context("created_lt disappeared")?)
            .into(),
        },
    )?;
    Ok(())
}

#[test]
#[ignore = "optional live TON Center contract test"]
fn config_param_request_covers_param_alias_and_seqno() -> Result<()> {
    let Some(live) = live()? else { return Ok(()) };
    let seqno = i32::try_from(masterchain_info(&live)?.last.seqno)?;

    for request in [
        v2::requests::ConfigParamRequest {
            param: Some(0.into()),
            config_id: None,
            seqno: None,
        },
        v2::requests::ConfigParamRequest {
            param: None,
            config_id: Some(0.into()),
            seqno: Some(seqno.into()),
        },
    ] {
        let _: v2::TonlibResponse<v2::responses::ConfigInfo> =
            live.get(&live.v2_url, "/getConfigParam", &request)?;
    }
    Ok(())
}

#[test]
#[ignore = "optional live TON Center contract test"]
fn config_all_request_covers_latest_and_explicit_seqno() -> Result<()> {
    let Some(live) = live()? else { return Ok(()) };
    let seqno = i32::try_from(masterchain_info(&live)?.last.seqno)?;

    for seqno in [None, Some(seqno.into())] {
        let _: v2::TonlibResponse<v2::responses::ConfigInfo> = live.get(
            &live.v2_url,
            "/getConfigAll",
            &v2::requests::ConfigAllRequest { seqno },
        )?;
    }
    Ok(())
}

#[test]
#[ignore = "optional live TON Center contract test"]
fn block_header_request_covers_id_and_hashes() -> Result<()> {
    let Some(live) = live()? else { return Ok(()) };
    let block = masterchain_info(&live)?.last;

    for include_hashes in [false, true] {
        let _: v2::TonlibResponse<v2::responses::BlockHeader> = live.get(
            &live.v2_url,
            "/getBlockHeader",
            &v2::requests::BlockHeaderRequest {
                workchain: i32::try_from(block.workchain)?.into(),
                shard: block.shard.clone().into(),
                seqno: i32::try_from(block.seqno)?.into(),
                root_hash: include_hashes.then(|| block.root_hash.clone()),
                file_hash: include_hashes.then(|| block.file_hash.clone()),
            },
        )?;
    }
    Ok(())
}

#[test]
#[ignore = "optional live TON Center contract test"]
fn block_data_request_returns_the_selected_block_boc() -> Result<()> {
    let Some(live) = live()? else { return Ok(()) };
    let block = masterchain_info(&live)?.last;
    let request = v2::requests::BlockDataRequest {
        workchain: i32::try_from(block.workchain)?.into(),
        shard: block.shard.clone().into(),
        seqno: i32::try_from(block.seqno)?.into(),
        root_hash: Some(block.root_hash.clone()),
        file_hash: Some(block.file_hash.clone()),
        archival: Some(true.into()),
    };

    let get_response: v2::TonlibResponse<v2::responses::BlockData> =
        live.get(&live.v2_url, "/getBlock", &request)?;
    let post_response: v2::TonlibResponse<v2::responses::BlockData> =
        live.post(&live.v2_url, "/getBlock", &request)?;

    anyhow::ensure!(get_response.result.type_tag == Default::default());
    anyhow::ensure!(get_response.result.id.root_hash == block.root_hash);
    anyhow::ensure!(get_response.result.id.file_hash == block.file_hash);
    anyhow::ensure!(get_response.result.data == post_response.result.data);
    Boc::decode_base64(&get_response.result.data).context("getBlock returned an invalid BoC")?;
    Ok(())
}

#[test]
#[ignore = "optional live TON Center contract test"]
fn lookup_block_request_covers_seqno_lt_and_unixtime() -> Result<()> {
    let Some(live) = live()? else { return Ok(()) };
    let block = &fixture(&live)?.block;
    let gen_utime = (block.gen_utime.to_string()).into();

    for request in [
        v2::requests::LookupBlockRequest {
            workchain: block.workchain.into(),
            shard: block.shard.clone().into(),
            seqno: Some(i32::try_from(block.seqno)?.into()),
            lt: None,
            unixtime: None,
        },
        v2::requests::LookupBlockRequest {
            workchain: block.workchain.into(),
            shard: block.shard.clone().into(),
            seqno: None,
            lt: Some(block.start_lt.clone().into()),
            unixtime: None,
        },
        v2::requests::LookupBlockRequest {
            workchain: block.workchain.into(),
            shard: block.shard.clone().into(),
            seqno: None,
            lt: None,
            unixtime: Some(gen_utime),
        },
    ] {
        let _: v2::TonlibResponse<v2::responses::TonBlockIdExt> =
            live.get(&live.v2_url, "/lookupBlock", &request)?;
    }
    Ok(())
}

#[test]
#[ignore = "optional live TON Center contract test"]
fn run_get_method_request_covers_latest_and_historical_state() -> Result<()> {
    let Some(live) = live()? else { return Ok(()) };
    let seqno = i32::try_from(masterchain_info(&live)?.last.seqno)?;

    for seqno in [None, Some(seqno)] {
        let _: v2::TonlibResponse<v2::responses::RunGetMethodResult> = live.post(
            &live.v2_url,
            "/runGetMethod",
            &v2::requests::RunGetMethodRequest::<v2::stack::LegacyStackEntry> {
                address: ELECTOR_ADDRESS.to_owned(),
                method: "participant_list_extended".into(),
                stack: Vec::new(),
                seqno,
            },
        )?;

        let _: v2::TonlibResponse<v2::responses::RunGetMethodStdResult> = live.post(
            &live.v2_url,
            "/runGetMethodStd",
            &v2::requests::RunGetMethodStdRequest {
                address: ELECTOR_ADDRESS.to_owned(),
                method: "participant_list_extended".into(),
                stack: Vec::new(),
                seqno: seqno.map(i64::from),
            },
        )?;
    }

    let boc = Boc::encode_base64(Cell::default());
    let _: v2::TonlibResponse<v2::responses::RunGetMethodStdResult> = live.post(
        &live.v2_url,
        "/runGetMethodStd",
        &v2::requests::RunGetMethodStdRequest {
            address: ELECTOR_ADDRESS.to_owned(),
            method: 1.into(),
            stack: vec![
                v2::stack::TvmStackEntry::number(7),
                v2::stack::TvmStackEntry::cell(boc.clone()),
                v2::stack::TvmStackEntry::slice(boc),
                v2::stack::TvmStackEntry::tuple(Vec::new()),
                v2::stack::TvmStackEntry::list(Vec::new()),
            ],
            seqno: None,
        },
    )?;
    Ok(())
}

#[test]
#[ignore = "optional live TON Center contract test"]
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
        let response: v2::TonlibResponse<v2::responses::MasterchainInfo> =
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
    let _: v2::TonlibResponse<v2::responses::Shards> = live.post(
        &live.v2_url,
        "/jsonRPC",
        &json!({"method": "getShards", "params": {"seqno": seqno}}),
    )?;
    Ok(())
}

#[test]
#[ignore = "optional live TON Center contract test"]
fn send_boc_request_deserializes_real_error_without_broadcasting() -> Result<()> {
    let Some(live) = live()? else { return Ok(()) };

    let response: TypedResponse<
        v2::TonlibResponse<v2::responses::ResultOk>,
        v2::TonlibErrorResponse,
    > = live.post_either(
        &live.v2_url,
        "/sendBoc",
        &v2::requests::SendBocRequest {
            boc: invalid_boc().to_owned(),
        },
    )?;
    match response {
        TypedResponse::Success(response) => {
            anyhow::bail!(
                "invalid BOC unexpectedly accepted: {:?}",
                response.result.type_tag
            )
        }
        TypedResponse::Error(_) => {}
    }

    let response: TypedResponse<
        v2::TonlibResponse<v2::responses::ExtMessageInfo>,
        v2::TonlibErrorResponse,
    > = live.post_either(
        &live.v2_url,
        "/sendBocReturnHash",
        &v2::requests::SendBocRequest {
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
#[ignore = "optional live TON Center contract test"]
fn json_rpc_typed_params_cover_address_information() -> Result<()> {
    let Some(live) = live()? else { return Ok(()) };
    let fixture = fixture(&live)?;

    let _: v2::TonlibResponse<v2::responses::AddressInformation> = live.post(
        &live.v2_url,
        "/jsonRPC",
        &v2::requests::JsonRpcRequest::<v2::stack::LegacyStackEntry> {
            jsonrpc: Some(json!("2.0")),
            id: Some(json!("live-address-information")),
            call: v2::requests::JsonRpcCall::GetAddressInformation(
                v2::requests::AddressInformationRequest {
                    address: fixture.transaction.account.clone(),
                    seqno: None,
                },
            ),
        },
    )?;

    Ok(())
}
