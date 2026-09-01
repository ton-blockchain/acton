use std::time::Duration;

use anyhow::Result;

use crate::{
    cli::NodeCommand,
    ton::{toolchain::Toolchain, tools::types::OperationContext},
};

pub async fn execute(command: NodeCommand) -> Result<()> {
    match command {
        NodeCommand::Stats { state } => {
            let toolchain = Toolchain::resolve(&state.state_dir, None).await?;
            let settings = toolchain.settings()?;
            let node = &settings.node;
            let node_layout = &toolchain.layout.node;
            let stats = toolchain
                .validator_console_tool
                .health(
                    &OperationContext::for_node(Duration::from_secs(20), &node.name),
                    &toolchain.validator_console_endpoint(node_layout, node),
                )
                .await?;
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "connection_ready": stats.connection_ready(),
                    "unix_time": stats.unix_time()?,
                    "masterchain_block_time": stats.masterchain_block_time()?,
                }))?
            );
        }
    }
    Ok(())
}
