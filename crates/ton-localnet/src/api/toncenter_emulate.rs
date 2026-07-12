use crate::api::toncenter_wallet;
use crate::localnet::LocalnetAccountState;
use crate::types::{Addr, BocBytes};
use anyhow::Context;
use ton::ton_core::cell::TonCell;
use ton::ton_core::traits::tlb::TLB;
use ton::ton_wallet::WalletVersion;
use ton_api::toncenter::emulate::v1::{TonConnectEmulateRequest, TonConnectMessage};
use ton_api::toncenter::v3::EstimateFeeRequest;
use tycho_types::boc::{Boc, BocRepr};
use tycho_types::cell::{Cell, CellSliceParts};
use tycho_types::models::{
    CurrencyCollection, ExtInMsgInfo, IntAddr, IntMsgInfo, MessageLayout, MsgInfo, OwnedMessage,
    StateInit, StdAddr, StdAddrFormat,
};
use tycho_types::num::Tokens;

const MAX_TON_CONNECT_MESSAGES: usize = 4;
const DEFAULT_VALID_UNTIL_SECONDS: u32 = 300;
const DUMMY_SIGNATURE: [u8; 64] = [0; 64];

pub(crate) fn validate_ton_connect_request(
    request: &TonConnectEmulateRequest,
) -> anyhow::Result<()> {
    if request.from.is_empty() {
        anyhow::bail!("from address is required for emulation");
    }
    if request.messages.is_empty() {
        anyhow::bail!("messages array cannot be empty");
    }
    if request.messages.len() > MAX_TON_CONNECT_MESSAGES {
        anyhow::bail!("messages array cannot contain more than 4 messages");
    }

    for (index, message) in request.messages.iter().enumerate() {
        if message.address.is_empty() {
            anyhow::bail!("message at index {index} cannot be empty");
        }
        if message.amount.is_empty() {
            anyhow::bail!("amount in message at index {index} cannot be empty");
        }
        message
            .amount
            .parse::<u64>()
            .with_context(|| format!("invalid amount in message at index {index}"))?;

        validate_optional_boc(message.payload.as_deref(), "payload", index)?;
        validate_optional_boc(message.state_init.as_deref(), "stateInit", index)?;
    }

    Ok(())
}

pub(crate) fn compose_ton_connect_message(
    request: &TonConnectEmulateRequest,
    account: &LocalnetAccountState,
    now: u32,
) -> anyhow::Result<BocBytes> {
    let (from, _) = StdAddr::from_str_ext(&request.from, StdAddrFormat::any())
        .context("Invalid from address format")?;
    let wallet = toncenter_wallet::read_standard_wallet_state(account)?;
    let version = wallet.version;
    if !matches!(
        version,
        WalletVersion::V3R1
            | WalletVersion::V3R2
            | WalletVersion::V4R1
            | WalletVersion::V4R2
            | WalletVersion::V5R1
    ) {
        anyhow::bail!("Unsupported wallet type: {version:?}");
    }
    let wallet_id = wallet.wallet_id.context("Wallet state has no wallet id")?;
    let valid_until = wallet_valid_until(version, request.valid_until, now)?;

    let messages = request
        .messages
        .iter()
        .map(|message| build_internal_message(&from, message, now))
        .collect::<anyhow::Result<Vec<_>>>()?;
    let body =
        WalletVersion::build_ext_in_body(version, valid_until, wallet.seqno, wallet_id, messages)
            .context("Failed to build wallet external message body")?;
    let signed_body = add_dummy_signature(version, body)?;
    let body = Boc::decode(signed_body.to_boc()?)
        .context("Failed to convert wallet body to local cell")?;

    let message = OwnedMessage {
        info: MsgInfo::ExtIn(ExtInMsgInfo {
            src: None,
            dst: IntAddr::Std(from),
            import_fee: Tokens::ZERO,
        }),
        init: None,
        body: CellSliceParts::from(body),
        layout: Some(MessageLayout::plain()),
    };
    Ok(BocRepr::encode(message)?.into())
}

pub(crate) fn compose_estimate_fee_message(
    request: &EstimateFeeRequest,
) -> anyhow::Result<BocBytes> {
    let destination = StdAddr::from(Addr::parse(&request.address)?);
    let body = decode_cell(&request.body, "body")?;
    let code = request
        .init_code
        .as_deref()
        .map(|value| decode_cell(value, "init_code"))
        .transpose()?;
    let data = request
        .init_data
        .as_deref()
        .map(|value| decode_cell(value, "init_data"))
        .transpose()?;
    let init = (code.is_some() || data.is_some()).then(|| StateInit {
        split_depth: None,
        special: None,
        code,
        data,
        libraries: Default::default(),
    });

    let message = OwnedMessage {
        info: MsgInfo::ExtIn(ExtInMsgInfo {
            src: None,
            dst: IntAddr::Std(destination),
            import_fee: Tokens::ZERO,
        }),
        init,
        body: CellSliceParts::from(body),
        layout: None,
    };
    Ok(BocRepr::encode(message)?.into())
}

fn validate_optional_boc(value: Option<&str>, field: &str, index: usize) -> anyhow::Result<()> {
    if let Some(value) = value {
        value
            .parse::<BocBytes>()
            .with_context(|| format!("invalid message {field} at index {index}"))?;
    }
    Ok(())
}

fn wallet_valid_until(
    version: WalletVersion,
    requested: Option<u64>,
    now: u32,
) -> anyhow::Result<u32> {
    if matches!(version, WalletVersion::V3R1 | WalletVersion::V3R2) {
        // TonCenter's V3 composer always uses a five-minute window.
        return now
            .checked_add(DEFAULT_VALID_UNTIL_SECONDS)
            .context("Default valid_until overflows u32");
    }

    requested.map_or_else(
        || {
            now.checked_add(DEFAULT_VALID_UNTIL_SECONDS)
                .context("Default valid_until overflows u32")
        },
        normalize_valid_until,
    )
}

fn normalize_valid_until(value: u64) -> anyhow::Result<u32> {
    let seconds = if value >= 1_000_000_000_000_000_000 {
        value / 1_000_000_000
    } else if value >= 1_000_000_000_000_000 {
        value / 1_000_000
    } else if value >= 1_000_000_000_000 {
        value / 1_000
    } else {
        value
    };
    u32::try_from(seconds).context("valid_until does not fit uint32 seconds")
}

fn build_internal_message(
    from: &StdAddr,
    message: &TonConnectMessage,
    now: u32,
) -> anyhow::Result<TonCell> {
    let (destination, flags) = StdAddr::from_str_ext(&message.address, StdAddrFormat::any())
        .context("Invalid destination address format")?;
    let amount = message
        .amount
        .parse::<u64>()
        .context("Invalid message amount")?;
    let body = decode_optional_cell(message.payload.as_deref())?.unwrap_or_default();
    let init = decode_optional_cell(message.state_init.as_deref())?
        .map(|cell| cell.parse::<StateInit>())
        .transpose()
        .context("Failed to parse message stateInit")?;

    let message = OwnedMessage {
        info: MsgInfo::Int(IntMsgInfo {
            ihr_disabled: true,
            bounce: flags.bounceable,
            bounced: false,
            src: IntAddr::Std(from.clone()),
            dst: IntAddr::Std(destination),
            value: CurrencyCollection::new(u128::from(amount)),
            ihr_fee: Default::default(),
            fwd_fee: Default::default(),
            created_lt: 0,
            created_at: now,
        }),
        init,
        body: CellSliceParts::from(body),
        layout: Some(MessageLayout {
            init_to_cell: message.state_init.is_some(),
            body_to_cell: message.payload.is_some(),
        }),
    };
    TonCell::from_boc(BocRepr::encode(message)?).context("Failed to build internal message")
}

fn decode_optional_cell(value: Option<&str>) -> anyhow::Result<Option<Cell>> {
    value
        .map(|value| {
            let boc = value.parse::<BocBytes>()?;
            Boc::decode(&boc.0).context("Failed to decode cell BOC")
        })
        .transpose()
}

fn decode_cell(value: &str, field: &str) -> anyhow::Result<Cell> {
    let boc = value
        .parse::<BocBytes>()
        .with_context(|| format!("invalid {field}"))?;
    Boc::decode(&boc.0).with_context(|| format!("failed to decode {field} BOC"))
}

fn add_dummy_signature(version: WalletVersion, body: TonCell) -> anyhow::Result<TonCell> {
    let mut builder = TonCell::builder();
    if version == WalletVersion::V5R1 {
        builder.write_cell(&body)?;
        builder.write_bits(DUMMY_SIGNATURE, DUMMY_SIGNATURE.len() * 8)?;
    } else {
        builder.write_bits(DUMMY_SIGNATURE, DUMMY_SIGNATURE.len() * 8)?;
        builder.write_cell(&body)?;
    }
    builder
        .build()
        .context("Failed to add dummy wallet signature")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::localnet::{LocalnetBlockId, LocalnetTransactionId};
    use crate::storage::AccountStatus;
    use crate::types::{Addr, Hash256};
    use ton::ton_core::cell::TonHash;
    use ton::ton_wallet::{
        WalletV1V2Data, WalletV3Data, WalletV3ExtMsgBody, WalletV4Data, WalletV4ExtMsgBody,
        WalletV5Data, WalletV5ExtMsgBody,
    };
    use tycho_types::cell::CellBuilder;
    use tycho_types::models::Message;

    const NOW: u32 = 1_700_000_000;
    const WALLET_ID: i32 = 0x1122_3344;
    const SEQNO: u32 = 7;

    #[test]
    fn composes_ton_connect_message_for_supported_wallets() -> anyhow::Result<()> {
        for version in [
            WalletVersion::V3R1,
            WalletVersion::V3R2,
            WalletVersion::V4R1,
            WalletVersion::V4R2,
            WalletVersion::V5R1,
        ] {
            let account = wallet_account(version)?;
            let request = ton_connect_request(Some(1_700_000_123_000));
            let boc = compose_ton_connect_message(&request, &account, NOW)?;
            let signed_body = extract_wallet_body(&boc)?;

            let (valid_until, messages, signature) = match version {
                WalletVersion::V3R1 | WalletVersion::V3R2 => {
                    let (body, signature) =
                        WalletV3ExtMsgBody::read_signed(&mut signed_body.parser())?;
                    assert_eq!(body.subwallet_id, WALLET_ID);
                    assert_eq!(body.msg_seqno, SEQNO);
                    (body.valid_until, body.msgs, signature)
                }
                WalletVersion::V4R1 | WalletVersion::V4R2 => {
                    let (body, signature) =
                        WalletV4ExtMsgBody::read_signed(&mut signed_body.parser())?;
                    assert_eq!(body.subwallet_id, WALLET_ID);
                    assert_eq!(body.msg_seqno, SEQNO);
                    assert_eq!(body.opcode, 0);
                    (body.valid_until, body.msgs, signature)
                }
                WalletVersion::V5R1 => {
                    let (body, signature) =
                        WalletV5ExtMsgBody::read_signed(&mut signed_body.parser())?;
                    assert_eq!(body.wallet_id, WALLET_ID);
                    assert_eq!(body.msg_seqno, SEQNO);
                    (body.valid_until, body.msgs, signature)
                }
                _ => unreachable!(),
            };

            let expected_valid_until =
                if matches!(version, WalletVersion::V3R1 | WalletVersion::V3R2) {
                    NOW + DEFAULT_VALID_UNTIL_SECONDS
                } else {
                    1_700_000_123
                };
            assert_eq!(valid_until, expected_valid_until);
            assert_eq!(signature, DUMMY_SIGNATURE);
            assert_eq!(messages.len(), 1);
            assert_internal_message(&messages[0])?;
        }
        Ok(())
    }

    #[test]
    fn rejects_unsupported_wallet_version() -> anyhow::Result<()> {
        let account = wallet_account(WalletVersion::V2R2)?;
        let error = compose_ton_connect_message(&ton_connect_request(None), &account, NOW)
            .expect_err("wallet V2 must be rejected");
        assert!(error.to_string().contains("Unsupported wallet type"));
        Ok(())
    }

    #[test]
    fn validates_ton_connect_request_shape() {
        let mut request = ton_connect_request(None);
        request.messages.clear();
        assert_eq!(
            validate_ton_connect_request(&request)
                .expect_err("empty messages must fail")
                .to_string(),
            "messages array cannot be empty"
        );

        let mut request = ton_connect_request(None);
        request.messages[0].amount = "-1".to_owned();
        assert!(
            validate_ton_connect_request(&request)
                .expect_err("negative amount must fail")
                .to_string()
                .contains("invalid amount in message at index 0")
        );

        let mut request = ton_connect_request(None);
        request.messages[0].payload = Some("not-base64".to_owned());
        assert!(
            validate_ton_connect_request(&request)
                .expect_err("invalid payload must fail")
                .to_string()
                .contains("invalid message payload at index 0")
        );

        let mut request = ton_connect_request(None);
        request.messages = vec![request.messages[0].clone(); MAX_TON_CONNECT_MESSAGES + 1];
        assert_eq!(
            validate_ton_connect_request(&request)
                .expect_err("too many messages must fail")
                .to_string(),
            "messages array cannot contain more than 4 messages"
        );
    }

    #[test]
    fn estimate_fee_accepts_base64_and_hex_bocs() -> anyhow::Result<()> {
        let body = CellBuilder::build_from(0xdead_beef_u32)?;
        let code = CellBuilder::build_from(0xcafe_babe_u32)?;
        let data = CellBuilder::build_from(0x1234_5678_u32)?;
        let address = format!("0:{}", "11".repeat(32));
        let base64 = EstimateFeeRequest {
            address: address.clone(),
            body: Boc::encode_base64(&body),
            init_code: Some(Boc::encode_base64(&code)),
            init_data: Some(Boc::encode_base64(&data)),
            ignore_chksig: None,
        };
        let hex = EstimateFeeRequest {
            address,
            body: Boc::encode_hex(&body),
            init_code: Some(Boc::encode_hex(&code)),
            init_data: Some(Boc::encode_hex(&data)),
            ignore_chksig: None,
        };

        assert_eq!(
            compose_estimate_fee_message(&base64)?,
            compose_estimate_fee_message(&hex)?
        );
        Ok(())
    }

    fn ton_connect_request(valid_until: Option<u64>) -> TonConnectEmulateRequest {
        let destination = StdAddr::new(0, [0x22; 32].into());
        let payload = Boc::encode_base64(Cell::default());
        let state_init = CellBuilder::build_from(StateInit::default()).expect("valid state init");
        TonConnectEmulateRequest {
            from: format!("0:{}", "11".repeat(32)),
            messages: vec![TonConnectMessage {
                address: destination.display_base64_url(true).to_string(),
                amount: "123456789".to_owned(),
                payload: Some(payload),
                state_init: Some(Boc::encode_base64(&state_init)),
            }],
            valid_until,
            include_code_data: false,
            include_address_book: false,
            include_metadata: false,
            with_actions: false,
            mc_block_seqno: None,
        }
    }

    fn wallet_account(version: WalletVersion) -> anyhow::Result<LocalnetAccountState> {
        let public_key = TonHash::from_slice_sized(&[0x33; 32]);
        let data = match version {
            WalletVersion::V2R2 => WalletV1V2Data::new(public_key).to_boc()?,
            WalletVersion::V3R1 | WalletVersion::V3R2 => WalletV3Data {
                seqno: SEQNO,
                wallet_id: WALLET_ID,
                public_key,
            }
            .to_boc()?,
            WalletVersion::V4R1 | WalletVersion::V4R2 => WalletV4Data {
                seqno: SEQNO,
                wallet_id: WALLET_ID,
                public_key,
                plugins: None,
            }
            .to_boc()?,
            WalletVersion::V5R1 => WalletV5Data {
                sign_allowed: true,
                seqno: SEQNO,
                wallet_id: WALLET_ID,
                public_key,
                extensions: None,
            }
            .to_boc()?,
            _ => unreachable!(),
        };
        let code = WalletVersion::get_code(version)?.clone();
        let code_hash = Hash256(*code.cell_hash()?.as_slice_sized());
        Ok(LocalnetAccountState {
            address: Addr {
                workchain: 0,
                addr: [0x11; 32],
            },
            account_state_hash: Hash256([0x44; 32]),
            balance: 1_000_000_000,
            code: Some(BocBytes(code.to_boc()?)),
            code_hash: Some(code_hash),
            data: Some(BocBytes(data)),
            data_hash: None,
            last_transaction_id: LocalnetTransactionId {
                lt: 1,
                hash: Hash256([0x55; 32]),
            },
            block_id: LocalnetBlockId::first(),
            state: AccountStatus::Active,
            sync_utime: u64::from(NOW),
            frozen_hash: None,
        })
    }

    fn extract_wallet_body(boc: &BocBytes) -> anyhow::Result<TonCell> {
        let message_cell = Boc::decode(&boc.0)?;
        let message = message_cell.parse::<Message<'_>>()?;
        let MsgInfo::ExtIn(info) = message.info else {
            anyhow::bail!("expected external-in message");
        };
        assert_eq!(info.dst, IntAddr::Std(StdAddr::new(0, [0x11; 32].into())));

        let mut builder = CellBuilder::new();
        builder.store_slice(message.body)?;
        TonCell::from_boc(Boc::encode(builder.build()?)).map_err(Into::into)
    }

    fn assert_internal_message(message: &TonCell) -> anyhow::Result<()> {
        let cell = Boc::decode(message.to_boc()?)?;
        let message = cell.parse::<Message<'_>>()?;
        let MsgInfo::Int(info) = message.info else {
            anyhow::bail!("expected internal message");
        };
        assert_eq!(u128::from(info.value.tokens), 123_456_789);
        assert!(info.bounce);
        assert!(message.init.is_some());
        assert_eq!(
            Boc::encode_base64(message.body.cell()),
            Boc::encode_base64(Cell::default())
        );
        Ok(())
    }
}
