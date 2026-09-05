//! Compose support for the localnet Docker runtime.

use super::COMPOSE_TEMPLATE;
use crate::{NetworkConfig, Node};

const NODE_TEMPLATE: &str = include_str!("../../assets/localton-node.compose.yaml");

// The template supplies the first line's indentation. Continuation lines stay
// at the command list's depth, while an empty replacement leaves a blank line.
const VALIDATOR_ARGS: &str = r"- --validator
      - --faucet
      - http://127.0.0.1:18000/faucet";

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
            &render_imported_account_args(&config.imported_account_bocs),
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
                .replace("__LOCALTON_IMAGE__", image)
                .replace("__LOCALTON_NODE_ID__", &node.id)
                .replace("__LOCALTON_NODE_PORT_BASE__", &node.port_base.to_string())
                .replace(
                    "__LOCALTON_VALIDATOR_ARGS__",
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

fn render_imported_account_args(imported_account_bocs: &[String]) -> String {
    imported_account_bocs
        .iter()
        .flat_map(|boc| ["- --add-account".to_owned(), format!("- \"{boc}\"")])
        .collect::<Vec<_>>()
        .join("\n      ")
}
