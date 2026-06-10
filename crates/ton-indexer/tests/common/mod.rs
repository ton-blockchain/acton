use expect_test::Expect;
use ton_indexer::actions::{Extraction, Trace, TraceNode, extract_actions, opcodes};

const SYNTHETIC_CONTRACT_CALL_OPCODE: u32 = 0xffff_fffe;

macro_rules! trace {
    ($source:literal) => {
        crate::common::parse_trace($source)
    };
}

pub(crate) use trace;

pub(crate) fn check_extraction(trace: Trace, expected: Expect) {
    let extraction = extract_actions(&trace);
    expected.assert_eq(&format_extraction(&extraction));
}

fn format_extraction(extraction: &Extraction) -> String {
    let actions = extraction
        .actions
        .iter()
        .map(|action| {
            format!(
                "{:?} nodes={:?} base_actions={:?}",
                action.kind, action.nodes, action.base_actions
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let base_actions = extraction
        .base_actions
        .iter()
        .map(|action| {
            format!(
                "#{} {:?} nodes={:?} root={} user_facing={}",
                action.id, action.kind, action.nodes, action.root_node, action.user_facing
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    format!("actions:\n{actions}\n\nbase_actions:\n{base_actions}\n")
}

pub(crate) fn parse_trace(source: &str) -> Trace {
    let lines = source
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect::<Vec<_>>();
    let common_indent = lines
        .iter()
        .map(|line| line.len() - line.trim_start().len())
        .min()
        .unwrap_or_default();
    let entries = lines
        .into_iter()
        .map(|line| &line[common_indent..])
        .map(parse_trace_line)
        .collect::<Vec<_>>();

    let mut position = 0;
    let root = build_trace_node(&entries, &mut position, 0);
    assert_eq!(position, entries.len(), "trace contains more than one root");

    Trace { root }
}

fn parse_trace_line(line: &str) -> (usize, TraceNode) {
    let Some((edge_start, edge_column)) = line
        .char_indices()
        .enumerate()
        .find_map(|(column, (byte, char))| matches!(char, '├' | '└').then_some((byte, column)))
    else {
        return (0, parse_trace_node(line));
    };

    let depth = edge_column / 4 + 1;
    let content = line[edge_start..]
        .trim_start_matches(['├', '└', '─'])
        .trim();

    (depth, parse_trace_node(content))
}

fn parse_trace_node(content: &str) -> TraceNode {
    let (opcode, id) = content
        .rsplit_once('#')
        .unwrap_or_else(|| panic!("trace line must end with #id: {content}"));
    let id = id
        .trim()
        .parse()
        .unwrap_or_else(|_| panic!("trace line has invalid #id: {content}"));
    let opcode = opcode.trim();

    TraceNode {
        id,
        opcode: normalize_opcode(opcode),
        children: Vec::new(),
    }
}

fn normalize_opcode(opcode: &str) -> Option<u32> {
    let opcode = opcode.trim();
    if opcode.is_empty() {
        return None;
    }

    if let Some(hex) = opcode
        .strip_prefix("0x")
        .or_else(|| opcode.strip_prefix("0X"))
    {
        return Some(
            u32::from_str_radix(hex, 16)
                .unwrap_or_else(|_| panic!("trace line has invalid opcode literal: {opcode}")),
        );
    }

    Some(match opcode {
        "DedustVaultNativeV2Swap" => opcodes::DEDUST_VAULT_NATIVE_V2_SWAP,
        "DedustPoolV2SwapExternal" => opcodes::DEDUST_POOL_V2_SWAP_EXTERNAL,
        "DedustPoolV2PayOutFromPool" => opcodes::DEDUST_POOL_V2_PAY_OUT_FROM_POOL,
        "DedustPoolV2SwapEvent" => opcodes::DEDUST_POOL_V2_SWAP_EVENT,
        "DedustPayout" => opcodes::DEDUST_PAYOUT,
        "DedustTonExcesses" => opcodes::DEDUST_TON_EXCESSES,
        "DedustTonPay" => opcodes::DEDUST_TON_PAY,
        "JettonTransfer" => opcodes::JETTON_TRANSFER,
        "JettonInternalTransfer" => opcodes::JETTON_INTERNAL_TRANSFER,
        "JettonNotify" => opcodes::JETTON_NOTIFY,
        "JettonWalletTransferNotification" => opcodes::JETTON_WALLET_TRANSFER_NOTIFICATION,
        "JettonMint" => opcodes::JETTON_MINT,
        "Excess" => opcodes::EXCESS,
        "PtonTonTransfer" => opcodes::PTON_WALLET_V2_TON_TRANSFER,
        "StonfiSwapV2" => opcodes::STONFI_SWAP_V2,
        "StonfiPayToV2" => opcodes::STONFI_PAY_TO_V2,
        "StonfiPayVaultV2" => opcodes::STONFI_PAY_VAULT_V2,
        "StonfiDepositRefFeeV2" => opcodes::STONFI_DEPOSIT_REF_FEE_V2,
        "WalletV5r1IncomingExternalMessage" => opcodes::WALLET_SIGNED_EXTERNAL_V5R1,
        "TextComment" => opcodes::TEXT_COMMENT,
        "PoolV3Swap" => opcodes::POOL_V3_SWAP,
        "PayTo" => opcodes::ROUTER_V3_PAY_TO,
        "RouterCall"
        | "SomeOtherCall"
        | "UnknownProtocolCall"
        | "WalletV4IncomingExternalMessage" => SYNTHETIC_CONTRACT_CALL_OPCODE,
        _ => panic!("trace line has unknown opcode name: {opcode}"),
    })
}

fn build_trace_node(
    entries: &[(usize, TraceNode)],
    position: &mut usize,
    expected_depth: usize,
) -> TraceNode {
    let Some((depth, node)) = entries.get(*position) else {
        panic!("expected node at depth {expected_depth}");
    };
    assert_eq!(*depth, expected_depth, "unexpected trace indentation");

    *position += 1;
    let mut node = node.clone();

    while entries
        .get(*position)
        .is_some_and(|(depth, _)| *depth > expected_depth)
    {
        node.children
            .push(build_trace_node(entries, position, expected_depth + 1));
    }

    node
}
