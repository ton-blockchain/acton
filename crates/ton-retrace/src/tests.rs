use crate::types::ComputeInfo;
use crate::{Network, retrace};
use std::collections::HashMap;
use toncenter_keys::{TONCENTER_MAINNET_API_KEY_ENV, TONCENTER_TESTNET_API_KEY_ENV};

#[tokio::test]
async fn test_retrace_709() {
    assert_retrace(
        Network::Mainnet,
        "3c1b02a33390e596d83b306eab57b3f7271bc90e2e527ea4cafccfde25139d41",
        709,
        false,
        true,
    )
    .await;
}

#[tokio::test]
async fn test_retrace_simple_0() {
    assert_retrace(
        Network::Mainnet,
        "9432b11f810c58b38658cbc41c52dd01cf3af18e950d375dcc867077554e4550",
        0,
        true,
        true,
    )
    .await;
}

#[tokio::test]
async fn test_retrace_single_exotic() {
    assert_retrace(
        Network::Mainnet,
        "4295a2c06ca9b0242d4b6638e4eb1a8da91a9d75dbeae4acc13a4355a4dd7a6a",
        0,
        true,
        true,
    )
    .await;
}

#[tokio::test]
async fn test_retrace_several_exotic() {
    assert_retrace(
        Network::Mainnet,
        "440e0490bd5efee08b23cf33e2cfd9b8d414c4cb717d3f92727fa49d4c51a09d",
        0,
        true,
        true,
    )
    .await;
}

#[tokio::test]
async fn test_retrace_wallet_v5_mismatch() {
    assert_retrace(
        Network::Mainnet,
        "d6b814f76ec8cae17664ceba18b978e510f2249b36a35bf7227db121c1516e96",
        0,
        true,
        true,
    )
    .await;
}

#[tokio::test]
async fn test_retrace_wallet_v4() {
    assert_retrace(
        Network::Mainnet,
        "f8b7a5b598c65ecb180338eec103bf28c199bf8346453342eb7022ccf2ea39f6",
        0,
        true,
        true,
    )
    .await;
}

#[tokio::test]
async fn test_retrace_uninit_state_init() {
    assert_retrace(
        Network::Mainnet,
        "5abe43cce74d536cdae76b989e55f7b37c61381308b8f1a4b8ecc3098c4b8b39",
        130,
        false,
        true,
    )
    .await;
}

#[tokio::test]
async fn test_retrace_exotic_in_msg() {
    assert_retrace(
        Network::Mainnet,
        "f64c6a3cdf3fad1d786aacf9a6130f18f3f76eeb71294f53bbd812ad3703e70a",
        0,
        true,
        true,
    )
    .await;
}

#[tokio::test]
async fn test_retrace_lib_load() {
    assert_retrace(
        Network::Mainnet,
        "a63b8b2f4b4493de5e67031ba3d65c7a8c0938ab56327608fb42bcbee901e4b7",
        0,
        true,
        true,
    )
    .await;
}

#[tokio::test]
async fn test_retrace_v12() {
    assert_retrace(
        Network::Testnet,
        "fadd5a2d53a26c4e8694e9e992c4f53f981655593b24847f19727c1140a255be",
        9,
        true,
        true,
    )
    .await;
}

#[allow(unsafe_code)]
async fn assert_retrace(
    net: Network,
    hash: &str,
    expected_exit_code: i32,
    expected_success: bool,
    expected_hash_ok: bool,
) {
    // SAFETY: well...
    unsafe {
        std::env::set_var(
            match net {
                Network::Mainnet => TONCENTER_MAINNET_API_KEY_ENV,
                Network::Testnet => TONCENTER_TESTNET_API_KEY_ENV,
                Network::Localnet | Network::Custom(_) => return,
            },
            "49efa980ccdcd018fd09d387e63537afd9db4dbb8509d69e7bc2303ca2b2c860",
        );
    }
    let result = retrace(net, hash, HashMap::default())
        .await
        .expect("Retrace failed");

    match result.emulated_tx.compute_info {
        ComputeInfo::Success {
            success, exit_code, ..
        } => {
            assert_eq!(
                exit_code, expected_exit_code,
                "Exit code mismatch for hash {hash}"
            );
            assert_eq!(
                success, expected_success,
                "Success status mismatch for hash {hash}"
            );
        }
        ComputeInfo::Skipped => {
            panic!("Compute phase was skipped for hash {hash}");
        }
    }

    assert_eq!(
        result.state_update_hash_ok, expected_hash_ok,
        "State update hash OK mismatch for hash {hash}"
    );
}
