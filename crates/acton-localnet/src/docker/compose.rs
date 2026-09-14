//! Compose support for the localnet Docker runtime.

use super::COMPOSE_TEMPLATE;
use crate::{NetworkConfig, Node};

const NODE_TEMPLATE: &str = include_str!("../../assets/localton-node.compose.yaml");

// Replace one valid YAML list item with the optional validator arguments.
const VALIDATOR_ARGS: &str = r"      - --validator
      - --faucet
      - http://127.0.0.1:18000/faucet
";

pub(super) fn render_compose(image: &str, config: &NetworkConfig, nodes: &[Node]) -> String {
    let ports = config.ports();

    COMPOSE_TEMPLATE
        .replace("__LOCALTON_IMAGE__", image)
        .replace("__LOCALTON_V2_PORT__", &ports.api_v2.to_string())
        .replace("__LOCALTON_V3_PORT__", &ports.api_v3.to_string())
        .replace("__LOCALTON_ADMIN_PORT__", &ports.admin.to_string())
        .replace("__LOCALTON_CONFIG_PORT__", &ports.config.to_string())
        .replace(
            "__LOCALTON_OBSERVABILITY_PORT__",
            &ports.observability.to_string(),
        )
        .replace(
            "__LOCALTON_BLOCK_TIME__",
            &config
                .block_time_ms
                .map_or_else(String::new, |milliseconds| {
                    format!("- --block-time\n      - \"{milliseconds}\"")
                }),
        )
        .replace(
            "__LOCALTON_ELECTION_TIME__",
            &config
                .election_time_seconds
                .map_or_else(String::new, |seconds| {
                    format!("- --election-time\n      - \"{seconds}\"")
                }),
        )
        .replace(
            "__LOCALTON_IMPORTED_ACCOUNTS__",
            &render_imported_account_args(
                config.imported_account_bocs.iter().chain(
                    config
                        .startup_wallets
                        .iter()
                        .map(|wallet| &wallet.shard_account_boc_hex),
                ),
            ),
        )
        .replace("__LOCALTON_JOIN_VOLUMES__", &render_join_volumes(nodes))
        .replace("__LOCALTON_JOIN_NODES__", &render_join_nodes(image, nodes))
}

fn render_join_nodes(image: &str, nodes: &[Node]) -> String {
    nodes
        .iter()
        .map(|node| {
            let name = serde_json::to_string(&node.name).expect("a node name is valid JSON");

            NODE_TEMPLATE
                // An explicitly stopped node must not return during a whole-network start.
                // Selecting its service directly still lets the start-node operation resume it.
                .replace(
                    "__LOCALTON_NODE_PROFILES__",
                    if node.stopped { "[\"stopped\"]" } else { "[]" },
                )
                .replace("__LOCALTON_IMAGE__", image)
                .replace("__LOCALTON_NODE_ID__", &node.id)
                .replace("__LOCALTON_NODE_PORT_BASE__", &node.port_base.to_string())
                .replace(
                    "      - __LOCALTON_VALIDATOR_ARGS__\n",
                    if node.validator { VALIDATOR_ARGS } else { "" },
                )
                // Insert user-controlled names last so marker-like text in a
                // quoted name cannot be interpreted as another template field.
                .replace("__LOCALTON_NODE_NAME__", &name)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn render_join_volumes(nodes: &[Node]) -> String {
    nodes
        .iter()
        .map(|node| format!("{}-state:", node.id))
        .collect::<Vec<_>>()
        .join("\n  ")
}

fn render_imported_account_args<'a>(
    imported_account_bocs: impl Iterator<Item = &'a String>,
) -> String {
    imported_account_bocs
        .flat_map(|boc| ["- --add-account".to_owned(), format!("- \"{boc}\"")])
        .collect::<Vec<_>>()
        .join("\n      ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::StartupWallet;

    #[test]
    fn genesis_contains_imports_and_startup_wallets_after_reload() {
        let config = NetworkConfig {
            port_base: 19000,
            ports: None,
            block_time_ms: None,
            election_time_seconds: None,
            imported_account_bocs: vec!["aabb".into()],
            startup_wallets: vec![StartupWallet {
                name: "deployer".into(),
                shard_account_boc_hex: "ccdd".into(),
            }],
        };
        let saved = serde_json::to_string(&config).unwrap();
        let restored = serde_json::from_str(&saved).unwrap();
        let compose = render_compose("localton", &restored, &[]);
        let accounts = compose
            .lines()
            .skip_while(|line| !line.contains("--add-account"))
            .take(4)
            .map(str::trim)
            .collect::<Vec<_>>()
            .join("\n");
        expect_test::expect![[r#"
            - --add-account
            - "aabb"
            - --add-account
            - "ccdd""#]]
        .assert_eq(&accounts);
    }
}
