use super::support::{Live, fixture, invalid_boc};
use anyhow::{Context, Result};
use std::env;
use ton_api::toncenter::emulate::v1;

fn live() -> Result<Option<Live>> {
    Live::from_env()
}

#[test]
#[ignore = "optional live TonCenter contract test"]
fn emulate_request_deserializes_real_validation_error() -> Result<()> {
    let Some(live) = live()? else { return Ok(()) };

    let _: v1::ErrorResponse = live.post_error(
        &live.emulate_url,
        "/emulateTrace",
        &v1::EmulateRequest {
            boc: invalid_boc().to_owned(),
            ignore_chksig: false,
            include_code_data: true,
            include_address_book: true,
            include_metadata: true,
            with_actions: true,
            mc_block_seqno: None,
        },
    )?;
    Ok(())
}

#[test]
#[ignore = "optional live TonCenter contract test"]
fn emulate_request_covers_success_response_and_option_variants() -> Result<()> {
    let Some(live) = live()? else { return Ok(()) };
    let Ok(boc) = env::var("ACTON_TONCENTER_LIVE_EMULATE_BOC") else {
        return Ok(());
    };
    let mc_block_seqno = fixture(&live)?.transaction.mc_block_seqno;

    for request in [
        v1::EmulateRequest {
            boc: boc.clone(),
            ignore_chksig: false,
            include_code_data: false,
            include_address_book: false,
            include_metadata: false,
            with_actions: false,
            mc_block_seqno: None,
        },
        v1::EmulateRequest {
            boc,
            ignore_chksig: true,
            include_code_data: true,
            include_address_book: true,
            include_metadata: true,
            with_actions: true,
            mc_block_seqno: Some(mc_block_seqno),
        },
    ] {
        let _: v1::EmulateTraceResponse =
            live.post(&live.emulate_url, "/emulateTrace", &request)?;
    }
    Ok(())
}

#[test]
#[ignore = "optional live TonCenter contract test"]
fn ton_connect_emulate_request_deserializes_real_validation_error() -> Result<()> {
    let Some(live) = live()? else { return Ok(()) };

    let _: v1::ErrorResponse = live.post_error(
        &live.emulate_url,
        "/emulateTonConnect",
        &v1::TonConnectEmulateRequest {
            from: String::new(),
            messages: vec![v1::TonConnectMessage {
                address: "not-an-address".to_owned(),
                amount: "not-an-amount".to_owned(),
                payload: Some(invalid_boc().to_owned()),
                state_init: Some(invalid_boc().to_owned()),
            }],
            valid_until: Some(0),
            include_code_data: true,
            include_address_book: true,
            include_metadata: true,
            with_actions: true,
            mc_block_seqno: None,
        },
    )?;
    Ok(())
}

#[test]
#[ignore = "optional live TonCenter contract test"]
fn ton_connect_emulate_request_covers_success_response() -> Result<()> {
    let Some(live) = live()? else { return Ok(()) };
    let Ok(raw_request) = env::var("ACTON_TONCENTER_LIVE_TONCONNECT_JSON") else {
        return Ok(());
    };
    let request: v1::TonConnectEmulateRequest = serde_json::from_str(&raw_request)
        .context("ACTON_TONCENTER_LIVE_TONCONNECT_JSON is not a typed TonConnect request")?;

    let _: v1::EmulateTraceResponse =
        live.post(&live.emulate_url, "/emulateTonConnect", &request)?;
    Ok(())
}
