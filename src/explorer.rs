use acton_config::config::Explorer;
use ton_api::Network;

pub(crate) fn actonscan_transaction_link(network: &Network, tx_hash_hex: &str) -> String {
    public_network_transaction_link(network, Explorer::Actonscan, "", tx_hash_hex, 0, 0)
}

pub(crate) fn public_network_transaction_link(
    network: &Network,
    explorer: Explorer,
    address: &str,
    tx_hash_hex: &str,
    lt: u64,
    utime: u32,
) -> String {
    let network_prefix = if network.uses_testnet_address_format() {
        "testnet."
    } else {
        ""
    };
    match explorer {
        Explorer::Actonscan if network.uses_testnet_address_format() => {
            format!("https://actonscan.com/tx/{tx_hash_hex}?network=testnet")
        }
        Explorer::Actonscan => {
            format!("https://actonscan.com/tx/{tx_hash_hex}?network=mainnet")
        }
        Explorer::Tonscan => format!("https://{network_prefix}tonscan.org/tx/{tx_hash_hex}"),
        Explorer::Toncx => {
            format!("https://{network_prefix}ton.cx/tx/{lt}:{tx_hash_hex}:{address}")
        }
        Explorer::Dton => format!("https://{network_prefix}dton.io/tx/{tx_hash_hex}?time={utime}"),
        Explorer::Tonviewer => {
            format!("https://{network_prefix}tonviewer.com/transaction/{tx_hash_hex}")
        }
    }
}
