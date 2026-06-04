mod common;

use common::{check_extraction, trace};
use expect_test::expect;

#[test]
fn dedust_swap_consumes_nested_jetton_transfer() {
    let trace = trace!(
        r"
        WalletV5r1IncomingExternalMessage #1
        └── DedustVaultNativeV2Swap #2
            └── DedustPoolV2SwapExternal #3
                ├── DedustPoolV2SwapEvent #4
                └── DedustPoolV2PayOutFromPool #5
                    └── JettonTransfer #6
                        └── JettonInternalTransfer #7
                            └── Excess #8
        "
    );

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
fn dedust_swap_leg_requires_immediate_edges() {
    let trace = trace!(
        r"
        WalletV5r1IncomingExternalMessage #1
        ├── DedustVaultNativeV2Swap #2
        │   └── RouterCall #3
        │       └── DedustPoolV2SwapExternal #4
        │           └── RouterCall #5
        │               └── DedustPoolV2PayOutFromPool #6
        └── DedustVaultNativeV2Swap #7
            └── DedustPoolV2SwapExternal #8
                └── RouterCall #9
                    └── DedustPoolV2PayOutFromPool #10
        "
    );

    check_extraction(
        trace,
        expect![[r"
            actions:
            ContractCall nodes={1} base_actions=[0]
            ContractCall nodes={2} base_actions=[1]
            ContractCall nodes={3} base_actions=[2]
            ContractCall nodes={4} base_actions=[3]
            ContractCall nodes={5} base_actions=[4]
            ContractCall nodes={6} base_actions=[5]
            ContractCall nodes={7} base_actions=[6]
            ContractCall nodes={8} base_actions=[7]
            ContractCall nodes={9} base_actions=[8]
            ContractCall nodes={10} base_actions=[9]

            base_actions:
            #0 ContractCall nodes={1} root=1 user_facing=true
            #1 ContractCall nodes={2} root=2 user_facing=true
            #2 ContractCall nodes={3} root=3 user_facing=true
            #3 ContractCall nodes={4} root=4 user_facing=true
            #4 ContractCall nodes={5} root=5 user_facing=true
            #5 ContractCall nodes={6} root=6 user_facing=true
            #6 ContractCall nodes={7} root=7 user_facing=true
            #7 ContractCall nodes={8} root=8 user_facing=true
            #8 ContractCall nodes={9} root=9 user_facing=true
            #9 ContractCall nodes={10} root=10 user_facing=true
        "]],
    );
}

#[test]
fn dedust_swap_event_must_be_immediate_to_be_consumed() {
    let trace = trace!(
        r"
        WalletV5r1IncomingExternalMessage #1
        └── DedustVaultNativeV2Swap #2
            └── DedustPoolV2SwapExternal #3
                ├── RouterCall #4
                │   └── DedustPoolV2SwapEvent #5
                └── DedustPoolV2PayOutFromPool #6
        "
    );

    check_extraction(
        trace,
        expect![[r"
            actions:
            ContractCall nodes={1} base_actions=[1]
            ContractCall nodes={4} base_actions=[2]
            ContractCall nodes={5} base_actions=[3]

            base_actions:
            #0 DedustNativeSwapLeg nodes={2, 3, 6} root=2 user_facing=false
            #1 ContractCall nodes={1} root=1 user_facing=true
            #2 ContractCall nodes={4} root=4 user_facing=true
            #3 ContractCall nodes={5} root=5 user_facing=true
        "]],
    );
}

#[test]
fn dedust_swap_requires_direct_jetton_transfer_action() {
    let trace = trace!(
        r"
        WalletV5r1IncomingExternalMessage #1
        └── DedustVaultNativeV2Swap #2
            └── DedustPoolV2SwapExternal #3
                └── DedustPoolV2PayOutFromPool #4
                    └── RouterCall #5
                        └── JettonTransfer #6
                            └── JettonInternalTransfer #7
        "
    );

    check_extraction(
        trace,
        expect![[r"
            actions:
            JettonTransfer nodes={6, 7} base_actions=[1]
            ContractCall nodes={1} base_actions=[2]
            ContractCall nodes={5} base_actions=[3]

            base_actions:
            #0 DedustNativeSwapLeg nodes={2, 3, 4} root=2 user_facing=false
            #1 JettonTransfer nodes={6, 7} root=6 user_facing=true
            #2 ContractCall nodes={1} root=1 user_facing=true
            #3 ContractCall nodes={5} root=5 user_facing=true
        "]],
    );
}

#[test]
fn dedust_swap_can_pay_out_to_pton_transfer() {
    let trace = trace!(
        r"
        DedustVaultNativeV2Swap #1
        └── DedustPoolV2SwapExternal #2
            └── DedustPoolV2PayOutFromPool #3
                └── JettonTransfer #4
                    ├── PtonTonTransfer #5
                    └── Excess #6
        "
    );

    check_extraction(
        trace,
        expect![[r"
            actions:
            DedustSwap nodes={1, 2, 3, 4, 5, 6} base_actions=[0, 1]

            base_actions:
            #0 DedustNativeSwapLeg nodes={1, 2, 3} root=1 user_facing=false
            #1 PtonTransfer nodes={4, 5, 6} root=4 user_facing=true
        "]],
    );
}

#[test]
fn dedust_jetton_swap_can_pay_out_native_ton() {
    let trace = trace!(
        r"
        DedustPoolV2SwapExternal #1
        ├── DedustPoolV2SwapEvent #2
        └── DedustPoolV2PayOutFromPool #3
            └── DedustPayout #4
        "
    );

    check_extraction(
        trace,
        expect![[r"
            actions:
            DedustSwap nodes={1, 2, 3, 4} base_actions=[0, 1]

            base_actions:
            #0 DedustJettonSwapLeg nodes={1, 2, 3} root=1 user_facing=false
            #1 DedustPayout nodes={4} root=4 user_facing=true
        "]],
    );
}

#[test]
fn dedust_jetton_swap_trace_extracts_native_payout_swap() {
    let trace = trace!(
        r"
        WalletV4IncomingExternalMessage #1
        └── JettonTransfer #2
            └── JettonInternalTransfer #3
                ├── JettonNotify #4
                │   ├── TextComment #5
                │   └── JettonTransfer #6
                │       └── JettonInternalTransfer #7
                │           ├── JettonNotify #8
                │           │   └── JettonTransfer #9
                │           │       └── JettonInternalTransfer #10
                │           │           ├── JettonNotify #11
                │           │           │   └── DedustPoolV2SwapExternal #12
                │           │           │       └── DedustPoolV2PayOutFromPool #13
                │           │           │           └── DedustPayout #14
                │           │           └── Excess #15
                │           └── Excess #16
                └── Excess #17
        "
    );

    check_extraction(
        trace,
        expect![[r"
            actions:
            DedustSwap nodes={12, 13, 14} base_actions=[3, 4]
            JettonTransfer nodes={2, 3, 4, 17} base_actions=[0]
            JettonTransfer nodes={6, 7, 8, 16} base_actions=[1]
            JettonTransfer nodes={9, 10, 11, 15} base_actions=[2]
            ContractCall nodes={1} base_actions=[5]
            ContractCall nodes={5} base_actions=[6]

            base_actions:
            #0 JettonTransfer nodes={2, 3, 4, 17} root=2 user_facing=true
            #1 JettonTransfer nodes={6, 7, 8, 16} root=6 user_facing=true
            #2 JettonTransfer nodes={9, 10, 11, 15} root=9 user_facing=true
            #3 DedustJettonSwapLeg nodes={12, 13} root=12 user_facing=false
            #4 DedustPayout nodes={14} root=14 user_facing=true
            #5 ContractCall nodes={1} root=1 user_facing=true
            #6 ContractCall nodes={5} root=5 user_facing=true
        "]],
    );
}

#[test]
fn stonfi_swap_consumes_direct_jetton_transfer_action() {
    let trace = trace!(
        r"
        StonfiSwapV2 #1
        └── StonfiPayToV2 #2
            └── JettonTransfer #3
                └── JettonInternalTransfer #4
        "
    );

    check_extraction(
        trace,
        expect![[r"
            actions:
            StonfiSwap nodes={1, 2, 3, 4} base_actions=[0, 1]

            base_actions:
            #0 StonfiSwap nodes={1, 2} root=1 user_facing=false
            #1 JettonTransfer nodes={3, 4} root=3 user_facing=true
        "]],
    );
}

#[test]
fn stonfi_swap_consumes_direct_pton_transfer_action() {
    let trace = trace!(
        r"
        StonfiSwapV2 #1
        └── StonfiPayToV2 #2
            └── JettonTransfer #3
                └── PtonTonTransfer #4
        "
    );

    check_extraction(
        trace,
        expect![[r"
            actions:
            StonfiSwap nodes={1, 2, 3, 4} base_actions=[0, 1]

            base_actions:
            #0 StonfiSwap nodes={1, 2} root=1 user_facing=false
            #1 PtonTransfer nodes={3, 4} root=3 user_facing=true
        "]],
    );
}

#[test]
fn stonfi_swap_requires_immediate_pay_to() {
    let trace = trace!(
        r"
        StonfiSwapV2 #1
        └── RouterCall #2
            └── StonfiPayToV2 #3
        "
    );

    check_extraction(
        trace,
        expect![[r"
            actions:
            ContractCall nodes={1} base_actions=[0]
            ContractCall nodes={2} base_actions=[1]
            ContractCall nodes={3} base_actions=[2]

            base_actions:
            #0 ContractCall nodes={1} root=1 user_facing=true
            #1 ContractCall nodes={2} root=2 user_facing=true
            #2 ContractCall nodes={3} root=3 user_facing=true
        "]],
    );
}

#[test]
fn stonfi_swap_requires_direct_payout_action() {
    let trace = trace!(
        r"
        StonfiSwapV2 #1
        └── StonfiPayToV2 #2
            └── RouterCall #3
                └── JettonTransfer #4
                    └── JettonInternalTransfer #5
        "
    );

    check_extraction(
        trace,
        expect![[r"
            actions:
            JettonTransfer nodes={4, 5} base_actions=[1]
            ContractCall nodes={3} base_actions=[2]

            base_actions:
            #0 StonfiSwap nodes={1, 2} root=1 user_facing=false
            #1 JettonTransfer nodes={4, 5} root=4 user_facing=true
            #2 ContractCall nodes={3} root=3 user_facing=true
        "]],
    );
}

#[test]
fn pton_transfer_matches_jetton_transfer_to_pton_ton_transfer() {
    let trace = trace!(
        r"
        JettonTransfer #1
        ├── PtonTonTransfer #2
        └── Excess #3
        "
    );

    check_extraction(
        trace,
        expect![[r"
            actions:
            PtonTransfer nodes={1, 2, 3} base_actions=[0]

            base_actions:
            #0 PtonTransfer nodes={1, 2, 3} root=1 user_facing=true
        "]],
    );
}

#[test]
fn pton_transfer_requires_immediate_pton_ton_transfer() {
    let trace = trace!(
        r"
        JettonTransfer #1
        └── RouterCall #2
            └── PtonTonTransfer #3
        "
    );

    check_extraction(
        trace,
        expect![[r"
            actions:
            ContractCall nodes={1} base_actions=[0]
            ContractCall nodes={2} base_actions=[1]
            ContractCall nodes={3} base_actions=[2]

            base_actions:
            #0 ContractCall nodes={1} root=1 user_facing=true
            #1 ContractCall nodes={2} root=2 user_facing=true
            #2 ContractCall nodes={3} root=3 user_facing=true
        "]],
    );
}

#[test]
fn stonfi_to_dedust_trace_extracts_nested_dedust_swap() {
    let trace = trace!(
        r"
        WalletV5r1IncomingExternalMessage #1
        ├── JettonTransfer #2
        │   └── JettonInternalTransfer #3
        │       ├── JettonNotify #4
        │       │   └── StonfiSwapV2 #5
        │       │       └── StonfiPayToV2 #6
        │       │           └── JettonTransfer #7
        │       │               ├── PtonTonTransfer #8
        │       │               │   └── DedustVaultNativeV2Swap #9
        │       │               │       └── DedustPoolV2SwapExternal #10
        │       │               │           └── DedustPoolV2PayOutFromPool #11
        │       │               │               └── JettonTransfer #12
        │       │               │                   └── JettonInternalTransfer #13
        │       │               │                       ├── JettonNotify #14
        │       │               │                       │   ├── 0x6d82d2a4 #15
        │       │               │                       │   │   └── Excess #16
        │       │               │                       │   └── JettonTransfer #17
        │       │               │                       │       └── JettonInternalTransfer #18
        │       │               │                       │           └── Excess #19
        │       │               │                       └── Excess #20
        │       │               └── Excess #21
        │       └── Excess #22
        └── 0x76dbd306 #23
        "
    );

    check_extraction(
        trace,
        expect![[r"
            actions:
            StonfiSwap nodes={5, 6, 7, 8, 21} base_actions=[1, 2]
            DedustSwap nodes={9, 10, 11, 12, 13, 14, 20} base_actions=[3, 4]
            JettonTransfer nodes={2, 3, 4, 22} base_actions=[0]
            JettonTransfer nodes={17, 18, 19} base_actions=[5]
            ContractCall nodes={1} base_actions=[6]
            ContractCall nodes={15} base_actions=[7]
            ContractCall nodes={16} base_actions=[8]
            ContractCall nodes={23} base_actions=[9]

            base_actions:
            #0 JettonTransfer nodes={2, 3, 4, 22} root=2 user_facing=true
            #1 StonfiSwap nodes={5, 6} root=5 user_facing=false
            #2 PtonTransfer nodes={7, 8, 21} root=7 user_facing=true
            #3 DedustNativeSwapLeg nodes={9, 10, 11} root=9 user_facing=false
            #4 JettonTransfer nodes={12, 13, 14, 20} root=12 user_facing=true
            #5 JettonTransfer nodes={17, 18, 19} root=17 user_facing=true
            #6 ContractCall nodes={1} root=1 user_facing=true
            #7 ContractCall nodes={15} root=15 user_facing=true
            #8 ContractCall nodes={16} root=16 user_facing=true
            #9 ContractCall nodes={23} root=23 user_facing=true
        "]],
    );
}

#[test]
fn standalone_jetton_transfer_stays_user_facing() {
    let trace = trace!(
        r"
        JettonTransfer #1
        ├── JettonInternalTransfer #2
        └── #3
        "
    );

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
    let trace = trace!(
        r"
        JettonTransfer #1
        ├── JettonInternalTransfer #2
        │   ├── JettonNotify #3
        │   └── Excess #4
        └── SomeOtherCall #5
        "
    );

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
    let trace = trace!(
        r"
        WalletV5r1IncomingExternalMessage #1
        └── JettonTransfer #2
            └── JettonInternalTransfer #3
                ├── JettonNotify #4
                │   └── JettonTransfer #5
                │       └── JettonInternalTransfer #6
                │           ├── JettonNotify #7
                │           └── Excess #8
                └── Excess #9
        "
    );

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
fn jetton_transfer_requires_immediate_internal_transfer() {
    let trace = trace!(
        r"
        JettonTransfer #1
        └── JettonNotify #2
            └── JettonInternalTransfer #3
        "
    );

    check_extraction(
        trace,
        expect![[r"
            actions:
            ContractCall nodes={1} base_actions=[0]
            ContractCall nodes={2} base_actions=[1]
            ContractCall nodes={3} base_actions=[2]

            base_actions:
            #0 ContractCall nodes={1} root=1 user_facing=true
            #1 ContractCall nodes={2} root=2 user_facing=true
            #2 ContractCall nodes={3} root=3 user_facing=true
        "]],
    );
}

#[test]
fn jetton_mint_covers_internal_transfer_side_effects() {
    let trace = trace!(
        r"
        WalletV5r1IncomingExternalMessage #1
        └── JettonMint #2
            └── JettonInternalTransfer #3
                ├── JettonWalletTransferNotification #4
                └── Excess #5
        "
    );

    check_extraction(
        trace,
        expect![[r"
            actions:
            JettonMint nodes={2, 3, 4, 5} base_actions=[0]
            ContractCall nodes={1} base_actions=[1]

            base_actions:
            #0 JettonMint nodes={2, 3, 4, 5} root=2 user_facing=true
            #1 ContractCall nodes={1} root=1 user_facing=true
        "]],
    );
}

#[test]
fn jetton_mint_requires_immediate_internal_transfer() {
    let trace = trace!(
        r"
        JettonMint #1
        └── JettonWalletTransferNotification #2
            └── JettonInternalTransfer #3
        "
    );

    check_extraction(
        trace,
        expect![[r"
            actions:
            ContractCall nodes={1} base_actions=[0]
            ContractCall nodes={2} base_actions=[1]
            ContractCall nodes={3} base_actions=[2]

            base_actions:
            #0 ContractCall nodes={1} root=1 user_facing=true
            #1 ContractCall nodes={2} root=2 user_facing=true
            #2 ContractCall nodes={3} root=3 user_facing=true
        "]],
    );
}

#[test]
fn fallback_distinguishes_ton_transfer_and_contract_call() {
    let trace = trace!(
        r"
        #1
        ├── UnknownProtocolCall #2
        ├── #3
        └── #4
        "
    );

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
