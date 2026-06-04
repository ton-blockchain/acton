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
        │           └── DedustPoolV2PayOutFromPool #5
        └── DedustVaultNativeV2Swap #6
            └── DedustPoolV2SwapExternal #7
                └── RouterCall #8
                    └── DedustPoolV2PayOutFromPool #9
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
