use crate::context::Wallet;
use acton_config::color::OwoColorize;
use anyhow::anyhow;
use ton_api::{SendBocError, SendBocErrorKind};

#[derive(Debug, Clone, Copy)]
pub enum SendBocContext<'a> {
    Generic,
    Wallet {
        wallet_name: &'a str,
        network_name: &'a str,
        seqno: u32,
        need_state_init: bool,
    },
}

impl<'a> SendBocContext<'a> {
    #[must_use]
    pub fn wallet(
        wallet: &'a Wallet,
        network_name: &'a str,
        seqno: u32,
        need_state_init: bool,
    ) -> Self {
        Self::Wallet {
            wallet_name: &wallet.name,
            network_name,
            seqno,
            need_state_init,
        }
    }
}

#[must_use]
pub fn format_send_boc_error(error: SendBocError, context: SendBocContext<'_>) -> anyhow::Error {
    anyhow!(render_send_boc_error(error.kind(), error.raw(), context,))
}

fn wallet_airdrop_command(wallet_name: &str, network_name: &str) -> Option<String> {
    match network_name {
        "testnet" => Some(format!("acton wallet airdrop {wallet_name}")),
        "localnet" => Some(format!("acton wallet airdrop {wallet_name} --net localnet")),
        _ => None,
    }
}

fn wallet_airdrop_fix_hint(wallet_name: &str, network_name: &str) -> String {
    wallet_airdrop_command(wallet_name, network_name)
        .map(|airdrop_command| {
            format!(
                r"

Possible fix:
- request funds to the wallet with {}",
                airdrop_command.yellow()
            )
        })
        .unwrap_or_default()
}

fn render_send_boc_error(kind: SendBocErrorKind, raw: &str, context: SendBocContext<'_>) -> String {
    match (context, kind) {
        (
            SendBocContext::Wallet {
                wallet_name,
                network_name,
                ..
            },
            SendBocErrorKind::MissingAccountState,
        ) => {
            let fix_hint = wallet_airdrop_fix_hint(wallet_name, network_name);
            let wallet_name = wallet_name.yellow();
            let network_name = network_name.cyan();
            format!(
                r"wallet {wallet_name} has no active state on network {network_name} and the deployment message was not accepted; likely causes:
- wallet is not deployed yet on {network_name}
- wallet configuration/address does not match {network_name}{fix_hint}"
            )
        },
        (
            SendBocContext::Wallet {
                wallet_name,
                network_name,
                seqno,
                need_state_init,
            },
            SendBocErrorKind::RejectedBeforeExecution,
        ) => {
            let fix_hint = if need_state_init {
                String::new()
            } else {
                wallet_airdrop_fix_hint(wallet_name, network_name)
            };
            let wallet_name = wallet_name.yellow();
            let network_name = network_name.cyan();
            let deployment_hint = if need_state_init {
                "- wallet deployment StateInit was invalid or rejected".to_string()
            } else {
                format!("- wallet is not deployed on {network_name}")
            };

            format!(
                r"wallet {wallet_name} rejected the external message before contract execution; likely causes:
- not enough balance to cover the transfer and fees
{deployment_hint}
- seqno is stale (message used seqno {seqno})
- message expired{fix_hint}"
            )
        }
        (SendBocContext::Generic, SendBocErrorKind::MissingAccountState) => {
            "external message was not accepted because the destination account has no active state or the supplied StateInit is invalid".to_string()
        }
        (SendBocContext::Generic, SendBocErrorKind::RejectedBeforeExecution) => {
            r"external message was rejected before contract execution; likely causes:
- not enough balance to cover the transfer and fees
- destination account is not deployed
- seqno is stale
- message expired"
                .to_string()
        }
        (_, SendBocErrorKind::Other) => raw.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::{SendBocContext, render_send_boc_error};
    use ton_api::SendBocErrorKind;

    #[test]
    fn wallet_missing_account_state_mentions_wallet_setup() {
        let rendered = render_send_boc_error(
            SendBocErrorKind::MissingAccountState,
            "raw toncenter error",
            SendBocContext::Wallet {
                wallet_name: "deployer",
                network_name: "testnet",
                seqno: 0,
                need_state_init: false,
            },
        );

        assert_eq!(
            rendered,
            r"wallet deployer has no active state on network testnet and the deployment message was not accepted; likely causes:
- wallet is not deployed yet on testnet
- wallet configuration/address does not match testnet

Possible fix:
- request funds to the wallet with acton wallet airdrop deployer",
        );
    }

    #[test]
    fn wallet_rejected_before_execution_mentions_seqno() {
        let rendered = render_send_boc_error(
            SendBocErrorKind::RejectedBeforeExecution,
            "raw toncenter error",
            SendBocContext::Wallet {
                wallet_name: "deployer",
                network_name: "testnet",
                seqno: 7,
                need_state_init: false,
            },
        );

        assert_eq!(
            rendered,
            r"wallet deployer rejected the external message before contract execution; likely causes:
- not enough balance to cover the transfer and fees
- wallet is not deployed on testnet
- seqno is stale (message used seqno 7)
- message expired

Possible fix:
- request funds to the wallet with acton wallet airdrop deployer",
        );
    }

    #[test]
    fn generic_unknown_error_preserves_raw_message() {
        let rendered = render_send_boc_error(
            SendBocErrorKind::Other,
            "raw toncenter error",
            SendBocContext::Generic,
        );

        assert_eq!(rendered, "raw toncenter error");
    }

    #[test]
    fn wallet_missing_account_state_on_localnet_mentions_localnet_airdrop() {
        let rendered = render_send_boc_error(
            SendBocErrorKind::MissingAccountState,
            "raw toncenter error",
            SendBocContext::Wallet {
                wallet_name: "deployer",
                network_name: "localnet",
                seqno: 0,
                need_state_init: false,
            },
        );

        assert_eq!(
            rendered,
            r"wallet deployer has no active state on network localnet and the deployment message was not accepted; likely causes:
- wallet is not deployed yet on localnet
- wallet configuration/address does not match localnet

Possible fix:
- request funds to the wallet with acton wallet airdrop deployer --net localnet",
        );
    }

    #[test]
    fn wallet_missing_account_state_on_custom_network_omits_airdrop_fix() {
        let rendered = render_send_boc_error(
            SendBocErrorKind::MissingAccountState,
            "raw toncenter error",
            SendBocContext::Wallet {
                wallet_name: "deployer",
                network_name: "mock-v2",
                seqno: 0,
                need_state_init: false,
            },
        );

        assert_eq!(
            rendered,
            r"wallet deployer has no active state on network mock-v2 and the deployment message was not accepted; likely causes:
- wallet is not deployed yet on mock-v2
- wallet configuration/address does not match mock-v2",
        );
    }
}
