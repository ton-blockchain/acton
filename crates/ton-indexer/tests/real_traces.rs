use anyhow::{Context, Result};
use expect_test::{Expect, expect};
use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;
use ton_api::{Network, TonApiClient, V3MessageSummary, V3Trace, V3TransactionSummary};
use ton_indexer::actions::{
    DecodedBody, DecodedValue, MessageFact, NodeFact, Trace, TraceFacts, TraceNode, enrich_actions,
    extract_actions, opcodes, render_action,
};
use tycho_types::boc::Boc;
use tycho_types::cell::Cell;
use tycho_types::models::{IntAddr, StdAddr, StdAddrFormat};

const LIVE_TESTS_ENV: &str = "TON_INDEXER_RUN_LIVE_TESTS";
const TESTNET_MINT_TX_HASH: &str =
    "b7c5c8fd81e297451c4dc89ba7ffae54b483775825b8db0655bd418217de0e0e";
const MAINNET_DEDUST_SWAP_TX_HASH: &str =
    "941b31936decc45a9206fcdbf50363dc4ae1f352e3914b91b883b45d350d324a";

#[test]
fn testnet_jetton_mint_trace_fetches_from_toncenter_and_renders_actions() -> Result<()> {
    run_live_trace_case(
        Network::Testnet,
        TESTNET_MINT_TX_HASH,
        "testnet mint",
        expect![[r"
            trace_id: t8XI/YHil0UcTcibp/+uVLSDd1gluNsGVb1BghfeDg4=

            actions:
            JettonMint nodes={2, 3, 4, 5} render=minted 100000000000 jetton units
            ContractCall nodes={1} render=called contract with opcode 0x7369676e

            base_actions:
            #0 JettonMint nodes={2, 3, 4, 5} root=2 user_facing=true
            #1 ContractCall nodes={1} root=1 user_facing=true
        "]],
    )
}

#[test]
fn mainnet_dedust_swap_trace_fetches_from_toncenter_and_renders_actions() -> Result<()> {
    run_live_trace_case(
        Network::Mainnet,
        MAINNET_DEDUST_SWAP_TX_HASH,
        "mainnet DeDust swap",
        expect![[r"
            trace_id: lBsxk23sxFqSBvzb9QNj3Erh81LjkUuRuIO0XTUNMko=

            actions:
            DedustSwap nodes={2, 4, 5, 6, 7, 8, 9} render=swapped 50000000000000 jetton units to 12.323063628 TON via DeDust
            ContractCall nodes={1} render=called contract with opcode 0xee547d7e
            ContractCall nodes={3} render=called contract with opcode 0x76dbd306

            base_actions:
            #0 JettonTransfer nodes={2, 4, 5, 6} root=2 user_facing=true
            #1 DedustJettonSwapLeg nodes={7, 8} root=7 user_facing=false
            #2 DedustPayout nodes={9} root=9 user_facing=true
            #3 ContractCall nodes={1} root=1 user_facing=true
            #4 ContractCall nodes={3} root=3 user_facing=true
        "]],
    )
}

fn run_live_trace_case(
    network: Network,
    tx_hash: &str,
    trace_name: &str,
    expected: Expect,
) -> Result<()> {
    if std::env::var_os(LIVE_TESTS_ENV).is_none() {
        eprintln!("skipping live Toncenter test; set {LIVE_TESTS_ENV}=1 to run it");
        return Ok(());
    }

    let client = TonApiClient::new(network, HashMap::new())?;
    let trace = client
        .get_traces_by_tx_hash(tx_hash, 1)?
        .into_iter()
        .next()
        .with_context(|| format!("{trace_name} trace must exist"))?;

    let formatted = format_trace_actions(&trace)?;
    println!("{formatted}");

    expected.assert_eq(&formatted);

    Ok(())
}

fn format_trace_actions(trace: &V3Trace) -> Result<String> {
    let (trace_model, facts) = actions_trace_from_toncenter(trace)?;
    let extraction = extract_actions(&trace_model);
    let enriched = enrich_actions(&extraction, &facts);

    let mut formatted = format!("trace_id: {}\n\nactions:\n", trace.trace_id);
    for action in &enriched {
        writeln!(
            formatted,
            "{:?} nodes={:?} render={}",
            action.action.kind,
            action.action.nodes,
            render_action(action),
        )?;
    }

    formatted.push_str("\nbase_actions:\n");
    for action in &extraction.base_actions {
        writeln!(
            formatted,
            "#{} {:?} nodes={:?} root={} user_facing={}",
            action.id, action.kind, action.nodes, action.root_node, action.user_facing,
        )?;
    }

    Ok(formatted)
}

fn actions_trace_from_toncenter(trace: &V3Trace) -> Result<(Trace, TraceFacts)> {
    let root_hash = trace
        .transactions_order
        .first()
        .context("Toncenter trace must contain at least one transaction")?;

    let node_ids = trace
        .transactions_order
        .iter()
        .enumerate()
        .map(|(index, hash)| (hash.clone(), (index + 1) as u64))
        .collect::<HashMap<_, _>>();
    let children_by_hash = child_transactions_by_hash(trace)?;

    let mut facts = TraceFacts::new();
    let root = build_trace_node(root_hash, trace, &node_ids, &children_by_hash, &mut facts)?;

    Ok((Trace { root }, facts))
}

fn child_transactions_by_hash(trace: &V3Trace) -> Result<HashMap<String, Vec<String>>> {
    let mut tx_by_in_msg_hash = HashMap::<String, String>::new();
    for tx_hash in &trace.transactions_order {
        let tx = transaction(trace, tx_hash)?;
        if let Some(in_msg_hash) = tx.in_msg.as_ref().and_then(|msg| msg.hash.clone()) {
            tx_by_in_msg_hash.insert(in_msg_hash, tx_hash.clone());
        }
    }

    let mut children = HashMap::<String, Vec<String>>::new();
    for tx_hash in &trace.transactions_order {
        let tx = transaction(trace, tx_hash)?;
        let tx_children = tx
            .out_msgs
            .iter()
            .filter_map(|msg| msg.hash.as_ref())
            .filter_map(|msg_hash| tx_by_in_msg_hash.get(msg_hash))
            .cloned()
            .collect::<Vec<_>>();
        children.insert(tx_hash.clone(), tx_children);
    }

    Ok(children)
}

fn build_trace_node(
    tx_hash: &str,
    trace: &V3Trace,
    node_ids: &HashMap<String, u64>,
    children_by_hash: &HashMap<String, Vec<String>>,
    facts: &mut TraceFacts,
) -> Result<TraceNode> {
    let tx = transaction(trace, tx_hash)?;
    let id = *node_ids
        .get(tx_hash)
        .with_context(|| format!("missing node id for tx {tx_hash}"))?;
    let fact = node_fact(id, tx.in_msg.as_ref());
    let opcode = fact.opcode;
    facts.insert(fact);

    let children = children_by_hash
        .get(tx_hash)
        .into_iter()
        .flatten()
        .map(|child_hash| build_trace_node(child_hash, trace, node_ids, children_by_hash, facts))
        .collect::<Result<Vec<_>>>()?;

    Ok(TraceNode {
        id,
        opcode,
        children,
    })
}

fn transaction<'a>(trace: &'a V3Trace, tx_hash: &str) -> Result<&'a V3TransactionSummary> {
    trace
        .transactions
        .get(tx_hash)
        .with_context(|| format!("missing transaction {tx_hash} in Toncenter trace payload"))
}

fn node_fact(id: u64, message: Option<&V3MessageSummary>) -> NodeFact {
    let decoded_body = message
        .and_then(|message| message.message_content.as_ref())
        .and_then(|content| content.body.as_deref())
        .and_then(decode_body);

    let (body, opcode, decoded) = decoded_body.unwrap_or((None, None, None));

    NodeFact {
        id,
        opcode,
        message: message.map(|message| MessageFact {
            source: parse_int_addr(message.source.as_deref()),
            destination: parse_int_addr(message.destination.as_deref()),
            value: message
                .value
                .as_deref()
                .and_then(|value| value.parse().ok())
                .unwrap_or_default(),
            bounced: message.bounced.unwrap_or_default(),
            body,
        }),
        decoded,
    }
}

fn decode_body(body: &str) -> Option<(Option<Cell>, Option<u32>, Option<DecodedBody>)> {
    let cell = Boc::decode_base64(body).ok()?;
    let mut slice = cell.as_slice_allow_exotic();
    let opcode = (slice.size_bits() >= 32)
        .then(|| slice.load_uint(32).ok().map(|opcode| opcode as u32))
        .flatten();
    let decoded = opcode.and_then(|opcode| decode_known_body(opcode, &mut slice));

    Some((Some(cell), opcode, decoded))
}

fn decode_known_body(
    opcode: u32,
    slice: &mut tycho_types::cell::CellSlice<'_>,
) -> Option<DecodedBody> {
    match opcode {
        opcodes::JETTON_TRANSFER
        | opcodes::JETTON_INTERNAL_TRANSFER
        | opcodes::JETTON_WALLET_TRANSFER_NOTIFICATION => {
            let type_name = match opcode {
                opcodes::JETTON_TRANSFER => "jetton_transfer",
                opcodes::JETTON_INTERNAL_TRANSFER => "jetton_internal_transfer",
                opcodes::JETTON_WALLET_TRANSFER_NOTIFICATION => "jetton_notify",
                _ => unreachable!(),
            };
            slice.load_uint(64).ok()?;
            let amount = load_var_uint16(slice)?;

            Some(DecodedBody {
                type_name: type_name.to_owned(),
                fields: BTreeMap::from([("amount".to_owned(), DecodedValue::Coins(amount))]),
            })
        }
        _ => None,
    }
}

fn load_var_uint16(slice: &mut tycho_types::cell::CellSlice<'_>) -> Option<u128> {
    let len = slice.load_uint(4).ok()? as usize;
    let mut value = 0u128;
    for _ in 0..len {
        value = (value << 8) | u128::from(slice.load_uint(8).ok()?);
    }
    Some(value)
}

fn parse_int_addr(raw: Option<&str>) -> Option<IntAddr> {
    let (addr, _) = StdAddr::from_str_ext(raw?, StdAddrFormat::any()).ok()?;
    Some(IntAddr::Std(addr))
}
