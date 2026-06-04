use expect_test::{Expect, expect};
use ton_indexer::actions::{Extraction, Trace, TraceNode, extract_actions};

macro_rules! node {
    ($id:literal, none) => {
        TraceNode {
            id: $id,
            opcode_name: None,
            children: Vec::new(),
        }
    };
    ($id:literal, none, [$($child:expr),* $(,)?]) => {
        TraceNode {
            id: $id,
            opcode_name: None,
            children: vec![$($child),*],
        }
    };
    ($id:literal, $opcode:literal) => {
        TraceNode {
            id: $id,
            opcode_name: Some($opcode.to_string()),
            children: Vec::new(),
        }
    };
    ($id:literal, $opcode:literal, [$($child:expr),* $(,)?]) => {
        TraceNode {
            id: $id,
            opcode_name: Some($opcode.to_string()),
            children: vec![$($child),*],
        }
    };
}

fn check_extraction(trace: Trace, expected: Expect) {
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

#[test]
fn dedust_swap_consumes_nested_jetton_transfer() {
    let trace = Trace {
        root: node!(
            1,
            "WalletV5r1IncomingExternalMessage",
            [node!(
                2,
                "DedustVaultNativeV2Swap",
                [node!(
                    3,
                    "DedustPoolV2SwapExternal",
                    [
                        node!(4, "DedustPoolV2SwapEvent"),
                        node!(
                            5,
                            "DedustPoolV2PayOutFromPool",
                            [node!(
                                6,
                                "JettonTransfer",
                                [node!(7, "JettonInternalTransfer", [node!(8, "Excess")])]
                            )]
                        ),
                    ]
                )]
            )]
        ),
    };

    check_extraction(
        trace,
        expect![[r"
            actions:
            DedustSwap nodes={2, 3, 4, 5, 6, 7, 8} base_actions=[0, 1]
            ContractCall nodes={1} base_actions=[2]

            base_actions:
            #0 DedustNativeSwapLeg nodes={2, 3, 4, 5} root=2 user_facing=false
            #1 JettonTransfer nodes={6, 7, 8} root=6 user_facing=true
            #2 ContractCall nodes={1} root=1 user_facing=true
        "]],
    );
}

#[test]
fn standalone_jetton_transfer_stays_user_facing() {
    let trace = Trace {
        root: node!(
            1,
            "JettonTransfer",
            [node!(2, "JettonInternalTransfer"), node!(3, none)]
        ),
    };

    check_extraction(
        trace,
        expect![[r"
            actions:
            JettonTransfer nodes={1, 2} base_actions=[0]
            TonTransfer nodes={3} base_actions=[1]

            base_actions:
            #0 JettonTransfer nodes={1, 2} root=1 user_facing=true
            #1 TonTransfer nodes={3} root=3 user_facing=true
        "]],
    );
}

#[test]
fn jetton_transfer_covers_optional_notify() {
    let trace = Trace {
        root: node!(
            1,
            "JettonTransfer",
            [
                node!(
                    2,
                    "JettonInternalTransfer",
                    [node!(3, "JettonNotify"), node!(4, "Excess")]
                ),
                node!(5, "SomeOtherCall"),
            ]
        ),
    };

    check_extraction(
        trace,
        expect![[r"
            actions:
            JettonTransfer nodes={1, 2, 3, 4} base_actions=[0]
            ContractCall nodes={5} base_actions=[1]

            base_actions:
            #0 JettonTransfer nodes={1, 2, 3, 4} root=1 user_facing=true
            #1 ContractCall nodes={5} root=5 user_facing=true
        "]],
    );
}

#[test]
fn jetton_notify_can_contain_nested_transfer() {
    let trace = Trace {
        root: node!(
            1,
            "WalletV5r1IncomingExternalMessage",
            [node!(
                2,
                "JettonTransfer",
                [node!(
                    3,
                    "JettonInternalTransfer",
                    [
                        node!(
                            4,
                            "JettonNotify",
                            [node!(
                                5,
                                "JettonTransfer",
                                [node!(
                                    6,
                                    "JettonInternalTransfer",
                                    [node!(7, "JettonNotify"), node!(8, "Excess")]
                                )]
                            )]
                        ),
                        node!(9, "Excess"),
                    ]
                )]
            )]
        ),
    };

    check_extraction(
        trace,
        expect![[r"
            actions:
            JettonTransfer nodes={2, 3, 4, 9} base_actions=[0]
            JettonTransfer nodes={5, 6, 7, 8} base_actions=[1]
            ContractCall nodes={1} base_actions=[2]

            base_actions:
            #0 JettonTransfer nodes={2, 3, 4, 9} root=2 user_facing=true
            #1 JettonTransfer nodes={5, 6, 7, 8} root=5 user_facing=true
            #2 ContractCall nodes={1} root=1 user_facing=true
        "]],
    );
}

#[test]
fn fallback_distinguishes_ton_transfer_and_contract_call() {
    let trace = Trace {
        root: node!(
            1,
            "",
            [
                node!(2, "UnknownProtocolCall"),
                node!(3, none),
                node!(4, "   "),
            ]
        ),
    };

    check_extraction(
        trace,
        expect![[r"
            actions:
            TonTransfer nodes={1} base_actions=[0]
            ContractCall nodes={2} base_actions=[1]
            TonTransfer nodes={3} base_actions=[2]
            TonTransfer nodes={4} base_actions=[3]

            base_actions:
            #0 TonTransfer nodes={1} root=1 user_facing=true
            #1 ContractCall nodes={2} root=2 user_facing=true
            #2 TonTransfer nodes={3} root=3 user_facing=true
            #3 TonTransfer nodes={4} root=4 user_facing=true
        "]],
    );
}
