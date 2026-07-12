use super::support::{Live, TypedResponse, fixture, invalid_boc};
use anyhow::Result;
use ton_api::toncenter::v3;

const ELECTOR_ADDRESS: &str = "-1:3333333333333333333333333333333333333333333333333333333333333333";
const WALLET_ADDRESS: &str = "0:5A488AA94CF819D3F7F86DA09C349C6E29CF018082D30B8B040A06F26929B284";
const NO_STATE_ADDRESS: &str = "0:0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF";
const USDT_MASTER: &str = "EQCxE6mUtQJKFnGfaROTKOt1lZbDiiX1kCixRv7Nw2Id_sDs";

fn live() -> Result<Option<Live>> {
    Live::from_env()
}

#[test]
#[ignore = "optional live TonCenter contract test"]
fn address_information_query_covers_v2_switch() -> Result<()> {
    let Some(live) = live()? else { return Ok(()) };
    let account = fixture(&live)?.transaction.account.clone();

    for use_v2 in [None, Some(false), Some(true)] {
        let _: v3::V2AddressInformation = live.get(
            &live.v3_url,
            "/addressInformation",
            &v3::AddressInformationQuery {
                address: account.clone(),
                use_v2,
            },
        )?;
    }
    Ok(())
}

#[test]
#[ignore = "optional live TonCenter contract test"]
fn wallet_information_query_covers_wallet_and_no_state_account() -> Result<()> {
    let Some(live) = live()? else { return Ok(()) };

    for address in [WALLET_ADDRESS, NO_STATE_ADDRESS] {
        for use_v2 in [None, Some(false), Some(true)] {
            let _: v3::V2WalletInformation = live.get(
                &live.v3_url,
                "/walletInformation",
                &v3::WalletInformationQuery {
                    address: address.to_owned(),
                    use_v2,
                },
            )?;
        }
    }
    Ok(())
}

#[test]
#[ignore = "optional live TonCenter contract test"]
fn masterchain_info_response_matches_typed_contract() -> Result<()> {
    let Some(live) = live()? else { return Ok(()) };
    let response: v3::MasterchainInfo = live.get(
        &live.v3_url,
        "/masterchainInfo",
        &v3::MasterchainInfoQuery::default(),
    )?;
    assert!(response.first.seqno <= response.last.seqno);
    Ok(())
}

#[test]
#[ignore = "optional live TonCenter contract test"]
fn address_book_query_covers_repeated_addresses() -> Result<()> {
    let Some(live) = live()? else { return Ok(()) };
    let response: v3::AddressBook = live.get(
        &live.v3_url,
        "/addressBook",
        &v3::AddressesQuery {
            address: vec![WALLET_ADDRESS.to_owned(), NO_STATE_ADDRESS.to_owned()],
        },
    )?;
    assert!(response.contains_key(WALLET_ADDRESS));
    assert!(response.contains_key(NO_STATE_ADDRESS));
    Ok(())
}

#[test]
#[ignore = "optional live TonCenter contract test"]
fn metadata_query_covers_token_and_plain_account() -> Result<()> {
    let Some(live) = live()? else { return Ok(()) };
    let response: v3::Metadata = live.get(
        &live.v3_url,
        "/metadata",
        &v3::AddressesQuery {
            address: vec![USDT_MASTER.to_owned(), WALLET_ADDRESS.to_owned()],
        },
    )?;
    assert!(response.values().any(|metadata| {
        metadata
            .token_info
            .iter()
            .any(|token| token.is_nsfw.is_some() && token.is_scam.is_some())
    }));
    Ok(())
}

#[test]
#[ignore = "optional live TonCenter contract test"]
fn transactions_by_masterchain_block_query_covers_pagination_and_sorting() -> Result<()> {
    let Some(live) = live()? else { return Ok(()) };
    let info: v3::MasterchainInfo = live.get(
        &live.v3_url,
        "/masterchainInfo",
        &v3::MasterchainInfoQuery::default(),
    )?;
    let _: v3::TransactionsResponse = live.get(
        &live.v3_url,
        "/transactionsByMasterchainBlock",
        &v3::TransactionsByMasterchainBlockQuery {
            seqno: i32::try_from(info.last.seqno.saturating_sub(10))?,
            limit: Some(2),
            offset: Some(0),
            sort: Some("desc".to_owned()),
        },
    )?;
    Ok(())
}

#[test]
#[ignore = "optional live TonCenter contract test"]
fn messages_query_covers_hash_addresses_ranges_directions_and_externals() -> Result<()> {
    let Some(live) = live()? else { return Ok(()) };
    let transaction = &fixture(&live)?.transaction;
    let (message, direction) = if let Some(message) = &transaction.in_msg {
        (message, "in")
    } else {
        let Some(message) = transaction.out_msgs.first() else {
            return Ok(());
        };
        (message, "out")
    };
    let opcode = message.opcode.as_ref().map(|value| match value {
        v3::StringOrNumber::String(value) => value.clone(),
        v3::StringOrNumber::Number(value) => value.to_string(),
        v3::StringOrNumber::Unsigned(value) => value.to_string(),
    });

    let response: v3::MessagesResponse = live.get(
        &live.v3_url,
        "/messages",
        &v3::MessagesQuery {
            msg_hash: vec![message.hash.clone()],
            body_hash: message
                .message_content
                .as_ref()
                .and_then(|content| content.hash.clone()),
            source: message.source.clone(),
            destination: message.destination.clone(),
            opcode,
            start_utime: Some(i32::try_from(transaction.now.saturating_sub(1))?),
            end_utime: Some(i32::try_from(transaction.now.saturating_add(1))?),
            start_lt: message.created_lt.as_deref().map(str::parse).transpose()?,
            end_lt: message.created_lt.as_deref().map(str::parse).transpose()?,
            direction: Some(direction.to_owned()),
            limit: Some(2),
            offset: Some(0),
            sort: Some("asc".to_owned()),
            ..Default::default()
        },
    )?;
    assert!(!response.messages.is_empty());

    let _: v3::MessagesResponse = live.get(
        &live.v3_url,
        "/messages",
        &v3::MessagesQuery {
            source: Some("null".to_owned()),
            only_externals: Some(true),
            limit: Some(1),
            ..Default::default()
        },
    )?;
    let _: v3::MessagesResponse = live.get(
        &live.v3_url,
        "/messages",
        &v3::MessagesQuery {
            exclude_externals: Some(true),
            limit: Some(1),
            sort: Some("desc".to_owned()),
            ..Default::default()
        },
    )?;
    Ok(())
}

#[test]
#[ignore = "optional live TonCenter contract test"]
fn adjacent_transactions_query_and_response_match_typed_contract() -> Result<()> {
    let Some(live) = live()? else { return Ok(()) };
    let transaction = &fixture(&live)?.transaction;
    let Some(in_message) = &transaction.in_msg else {
        return Ok(());
    };
    let messages: v3::MessagesResponse = live.get(
        &live.v3_url,
        "/messages",
        &v3::MessagesQuery {
            msg_hash: vec![in_message.hash.clone()],
            limit: Some(1),
            ..Default::default()
        },
    )?;
    let Some(message) = messages.messages.first() else {
        return Ok(());
    };
    let Some(hash) = message.in_msg_tx_hash.clone() else {
        return Ok(());
    };

    for direction in [None, Some("in".to_owned()), Some("out".to_owned())] {
        let _: TypedResponse<v3::TransactionsResponse, v3::RequestError> = live.get_either(
            &live.v3_url,
            "/adjacentTransactions",
            &v3::AdjacentTransactionsQuery {
                hash: hash.clone(),
                direction,
            },
        )?;
    }
    Ok(())
}

#[test]
#[ignore = "optional live TonCenter contract test"]
fn wallet_states_query_covers_wallet_contract_and_no_state_account() -> Result<()> {
    let Some(live) = live()? else { return Ok(()) };
    let response: v3::WalletStatesResponse = live.get(
        &live.v3_url,
        "/walletStates",
        &v3::WalletStatesQuery {
            address: vec![
                WALLET_ADDRESS.to_owned(),
                USDT_MASTER.to_owned(),
                NO_STATE_ADDRESS.to_owned(),
            ],
        },
    )?;
    assert!(response.wallets.iter().any(|wallet| wallet.is_wallet));
    assert!(response.wallets.iter().any(|wallet| !wallet.is_wallet));
    assert!(
        response
            .wallets
            .iter()
            .all(|wallet| wallet.address != NO_STATE_ADDRESS)
    );
    Ok(())
}

#[test]
#[ignore = "optional live TonCenter contract test"]
fn top_accounts_by_balance_query_and_response_match_typed_contract() -> Result<()> {
    let Some(live) = live()? else { return Ok(()) };
    let accounts: Vec<v3::AccountBalance> = live.get(
        &live.v3_url,
        "/topAccountsByBalance",
        &v3::TopAccountsByBalanceQuery {
            limit: Some(2),
            offset: Some(0),
        },
    )?;
    assert!(!accounts.is_empty());
    for pair in accounts.windows(2) {
        assert!(pair[0].balance.parse::<u128>()? >= pair[1].balance.parse::<u128>()?);
    }
    Ok(())
}

#[test]
#[ignore = "optional live TonCenter contract test"]
fn estimate_fee_request_and_response_match_typed_contract() -> Result<()> {
    let Some(live) = live()? else { return Ok(()) };
    let messages: v3::MessagesResponse = live.get(
        &live.v3_url,
        "/messages",
        &v3::MessagesQuery {
            source: Some("null".to_owned()),
            only_externals: Some(true),
            limit: Some(20),
            sort: Some("desc".to_owned()),
            ..Default::default()
        },
    )?;
    let Some((address, body)) = messages.messages.iter().find_map(|message| {
        Some((
            message.destination.clone()?,
            message.message_content.as_ref()?.body.clone()?,
        ))
    }) else {
        return Ok(());
    };

    for ignore_chksig in [None, Some(false), Some(true)] {
        let response: v3::EstimateFeeResult = live.post(
            &live.v3_url,
            "/estimateFee",
            &v3::EstimateFeeRequest {
                address: address.clone(),
                body: body.clone(),
                init_code: None,
                init_data: None,
                ignore_chksig,
            },
        )?;
        assert!(response.source_fees.in_fwd_fee > 0);
    }
    Ok(())
}

#[test]
#[ignore = "optional live TonCenter contract test"]
fn account_states_query_covers_repeated_addresses_and_boc() -> Result<()> {
    let Some(live) = live()? else { return Ok(()) };
    let fixture = fixture(&live)?;

    for request in [
        v3::AccountStatesQuery {
            address: vec![fixture.transaction.account.clone()],
            include_boc: Some(false),
        },
        v3::AccountStatesQuery {
            address: vec![
                fixture.transaction.account.clone(),
                ELECTOR_ADDRESS.to_owned(),
            ],
            include_boc: Some(true),
        },
    ] {
        let _: v3::AccountStatesResponse = live.get(&live.v3_url, "/accountStates", &request)?;
    }
    Ok(())
}

#[test]
#[ignore = "optional live TonCenter contract test"]
fn traces_query_covers_hash_account_ranges_actions_and_sorting() -> Result<()> {
    let Some(live) = live()? else { return Ok(()) };
    let transaction = &fixture(&live)?.transaction;
    let now = i32::try_from(transaction.now)?;
    let mc_seqno = Some(i32::try_from(transaction.mc_block_seqno)?);

    let mut requests = vec![
        v3::TracesQuery {
            tx_hash: vec![transaction.hash.clone()],
            include_actions: Some(true),
            limit: Some(2),
            sort: Some("desc".to_owned()),
            ..Default::default()
        },
        v3::TracesQuery {
            account: Some(transaction.account.clone()),
            mc_seqno,
            start_utime: Some(now.saturating_sub(60)),
            end_utime: Some(now.saturating_add(60)),
            start_lt: Some(transaction.lt.parse()?),
            end_lt: Some(transaction.lt.parse()?),
            include_actions: Some(false),
            supported_action_types: vec!["ton_transfer".to_owned()],
            limit: Some(2),
            offset: Some(0),
            sort: Some("asc".to_owned()),
            ..Default::default()
        },
    ];
    if let Some(hash) = transaction
        .in_msg
        .as_ref()
        .map(|message| message.hash.clone())
    {
        requests.push(v3::TracesQuery {
            msg_hash: vec![hash],
            limit: Some(2),
            ..Default::default()
        });
    }

    for request in requests {
        let _: v3::TracesResponse = live.get(&live.v3_url, "/traces", &request)?;
    }
    Ok(())
}

#[test]
#[ignore = "optional live TonCenter contract test"]
fn transactions_query_covers_hash_block_account_ranges_and_exclusion() -> Result<()> {
    let Some(live) = live()? else { return Ok(()) };
    let fixture = fixture(&live)?;
    let transaction = &fixture.transaction;
    let block = &fixture.block;
    let now = i32::try_from(transaction.now)?;
    let mc_seqno = Some(i32::try_from(transaction.mc_block_seqno)?);

    for request in [
        v3::TransactionsQuery {
            hash: Some(transaction.hash.clone()),
            lt: Some(transaction.lt.parse()?),
            limit: Some(2),
            ..Default::default()
        },
        v3::TransactionsQuery {
            account: vec![transaction.account.clone()],
            start_utime: Some(now.saturating_sub(60)),
            end_utime: Some(now.saturating_add(60)),
            start_lt: Some(transaction.lt.parse()?),
            end_lt: Some(transaction.lt.parse()?),
            limit: Some(2),
            offset: Some(0),
            sort: Some("asc".to_owned()),
            ..Default::default()
        },
        v3::TransactionsQuery {
            workchain: Some(block.workchain),
            shard: Some(block.shard.clone()),
            seqno: Some(i32::try_from(block.seqno)?),
            mc_seqno,
            exclude_account: vec![ELECTOR_ADDRESS.to_owned()],
            limit: Some(2),
            sort: Some("desc".to_owned()),
            ..Default::default()
        },
    ] {
        let _: v3::TransactionsResponse = live.get(&live.v3_url, "/transactions", &request)?;
    }
    Ok(())
}

#[test]
#[ignore = "optional live TonCenter contract test"]
fn blocks_query_covers_hash_block_ranges_and_sorting() -> Result<()> {
    let Some(live) = live()? else { return Ok(()) };
    let block = &fixture(&live)?.block;
    let start_lt = block.start_lt.parse()?;
    let end_lt = block.end_lt.parse()?;
    let gen_utime = block.gen_utime.to_bigint()?.to_string().parse()?;

    for request in [
        v3::BlocksQuery {
            workchain: Some(block.workchain),
            shard: Some(block.shard.clone()),
            seqno: Some(i32::try_from(block.seqno)?),
            root_hash: Some(block.root_hash.clone()),
            file_hash: Some(block.file_hash.clone()),
            limit: Some(2),
            ..Default::default()
        },
        v3::BlocksQuery {
            start_utime: Some(gen_utime),
            end_utime: Some(gen_utime),
            start_lt: Some(start_lt),
            end_lt: Some(end_lt),
            limit: Some(2),
            offset: Some(0),
            sort: Some("asc".to_owned()),
            ..Default::default()
        },
    ] {
        let _: v3::BlocksResponse = live.get(&live.v3_url, "/blocks", &request)?;
    }
    Ok(())
}

#[test]
#[ignore = "optional live TonCenter contract test"]
fn transactions_by_message_query_covers_hash_body_opcode_and_direction() -> Result<()> {
    let Some(live) = live()? else { return Ok(()) };
    let transaction = &fixture(&live)?.transaction;
    let (message, direction) = if let Some(message) = &transaction.in_msg {
        (message, "in")
    } else {
        let Some(message) = transaction.out_msgs.first() else {
            return Ok(());
        };
        (message, "out")
    };

    let _: v3::TransactionsResponse = live.get(
        &live.v3_url,
        "/transactionsByMessage",
        &v3::TransactionsByMessageQuery {
            msg_hash: Some(message.hash.clone()),
            direction: Some(direction.to_owned()),
            limit: Some(2),
            offset: Some(0),
            ..Default::default()
        },
    )?;
    if let Some(content) = &message.message_content {
        let _: v3::TransactionsResponse = live.get(
            &live.v3_url,
            "/transactionsByMessage",
            &v3::TransactionsByMessageQuery {
                body_hash: content.hash.clone(),
                opcode: message.opcode.as_ref().map(|value| match value {
                    v3::StringOrNumber::String(value) => value.clone(),
                    v3::StringOrNumber::Number(value) => value.to_string(),
                    v3::StringOrNumber::Unsigned(value) => value.to_string(),
                }),
                limit: Some(2),
                ..Default::default()
            },
        )?;
    }
    Ok(())
}

#[test]
#[ignore = "optional live TonCenter contract test"]
fn pending_transactions_query_covers_account_and_trace_filters() -> Result<()> {
    let Some(live) = live()? else { return Ok(()) };
    let transaction = &fixture(&live)?.transaction;

    for request in [
        v3::PendingTransactionsQuery {
            account: vec![transaction.account.clone()],
            trace_id: Vec::new(),
        },
        v3::PendingTransactionsQuery {
            account: vec![transaction.account.clone()],
            trace_id: transaction.trace_id.clone().into_iter().collect(),
        },
    ] {
        let _: TypedResponse<v3::TransactionsResponse, v3::RequestError> =
            live.get_either(&live.v3_url, "/pendingTransactions", &request)?;
    }
    Ok(())
}

#[test]
#[ignore = "optional live TonCenter contract test"]
fn jetton_masters_query_covers_pagination_address_and_admin() -> Result<()> {
    let Some(live) = live()? else { return Ok(()) };
    let initial: v3::JettonMastersResponse = live.get(
        &live.v3_url,
        "/jetton/masters",
        &v3::JettonMastersQuery {
            limit: Some(2),
            offset: Some(0),
            ..Default::default()
        },
    )?;

    if let Some(master) = initial.jetton_masters.first() {
        let _: v3::JettonMastersResponse = live.get(
            &live.v3_url,
            "/jetton/masters",
            &v3::JettonMastersQuery {
                address: vec![master.address.clone()],
                admin_address: master.admin_address.clone().into_iter().collect(),
                limit: Some(2),
                ..Default::default()
            },
        )?;
    }
    Ok(())
}

#[test]
#[ignore = "optional live TonCenter contract test"]
fn jetton_wallets_query_covers_filters_balance_and_sorting() -> Result<()> {
    let Some(live) = live()? else { return Ok(()) };
    let masters: v3::JettonMastersResponse = live.get(
        &live.v3_url,
        "/jetton/masters",
        &v3::JettonMastersQuery {
            limit: Some(1),
            ..Default::default()
        },
    )?;
    let Some(master) = masters.jetton_masters.first() else {
        return Ok(());
    };
    let initial: v3::JettonWalletsResponse = live.get(
        &live.v3_url,
        "/jetton/wallets",
        &v3::JettonWalletsQuery {
            jetton_address: vec![master.address.clone()],
            exclude_zero_balance: Some(false),
            limit: Some(2),
            offset: Some(0),
            sort: Some("desc".to_owned()),
            ..Default::default()
        },
    )?;

    if let Some(wallet) = initial.jetton_wallets.first() {
        let _: v3::JettonWalletsResponse = live.get(
            &live.v3_url,
            "/jetton/wallets",
            &v3::JettonWalletsQuery {
                address: vec![wallet.address.clone()],
                owner_address: vec![wallet.owner.clone()],
                jetton_address: vec![wallet.jetton.clone()],
                exclude_zero_balance: Some(true),
                limit: Some(2),
                sort: Some("asc".to_owned()),
                ..Default::default()
            },
        )?;
    }
    Ok(())
}

#[test]
#[ignore = "optional live TonCenter contract test"]
fn nft_items_query_covers_filters_sale_and_sorting() -> Result<()> {
    let Some(live) = live()? else { return Ok(()) };
    let initial: v3::NftItemsResponse = live.get(
        &live.v3_url,
        "/nft/items",
        &v3::NftItemsQuery {
            include_on_sale: Some(true),
            sort_by_last_transaction_lt: Some(true),
            limit: Some(2),
            offset: Some(0),
            ..Default::default()
        },
    )?;

    if let Some(item) = initial.nft_items.first() {
        let _: v3::NftItemsResponse = live.get(
            &live.v3_url,
            "/nft/items",
            &v3::NftItemsQuery {
                address: vec![item.address.clone()],
                owner_address: item.owner_address.clone().into_iter().collect(),
                collection_address: item.collection_address.clone().into_iter().collect(),
                index: vec![item.index.clone()],
                include_on_sale: Some(false),
                sort_by_last_transaction_lt: Some(false),
                limit: Some(2),
                ..Default::default()
            },
        )?;
    }
    Ok(())
}

#[test]
#[ignore = "optional live TonCenter contract test"]
fn send_message_request_deserializes_real_error_without_broadcasting() -> Result<()> {
    let Some(live) = live()? else { return Ok(()) };

    let response: TypedResponse<v3::SendMessageResult, v3::RequestError> = live.post_either(
        &live.v3_url,
        "/message",
        &v3::SendMessageRequest {
            boc: invalid_boc().to_owned(),
        },
    )?;
    match response {
        TypedResponse::Success(response) => {
            anyhow::bail!(
                "invalid BOC unexpectedly accepted: {}",
                response.message_hash
            )
        }
        TypedResponse::Error(_) => Ok(()),
    }
}

#[test]
#[ignore = "optional live TonCenter contract test"]
fn run_get_method_request_and_response() -> Result<()> {
    let Some(live) = live()? else { return Ok(()) };

    let response: v3::RunGetMethodResult = live.post(
        &live.v3_url,
        "/runGetMethod",
        &v3::RunGetMethodRequest {
            address: ELECTOR_ADDRESS.to_owned(),
            method: "participant_list_extended".to_owned(),
            stack: Vec::new(),
        },
    )?;
    if response.exit_code != 0 {
        anyhow::bail!(
            "elector get method returned exit code {}",
            response.exit_code
        );
    }
    Ok(())
}
