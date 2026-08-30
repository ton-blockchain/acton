use std::{path::Path, time::Duration};

use anyhow::Result;
use serde::Serialize;

use crate::{
    cli::NodeCommand,
    storage::Layout,
    storage::{NodeRuntime, RuntimeState},
    storage::{NodeSettings, Settings},
    ton::{toolchain::Toolchain, tools::types::OperationContext},
};

#[derive(Debug, Serialize)]
struct NodeView {
    settings: NodeSettings,
    runtime: Option<NodeRuntime>,
}

pub async fn execute(command: NodeCommand) -> Result<()> {
    match command {
        NodeCommand::List { state } => {
            let layout = layout(&state.state_dir)?;
            let settings = Settings::load_or_create(&layout.settings)?;
            let runtime = RuntimeState::load(&layout.runtime)?;
            let nodes: Vec<_> = settings
                .nodes
                .into_iter()
                .map(|settings| NodeView {
                    runtime: runtime.nodes.get(&settings.name).cloned(),
                    settings,
                })
                .collect();
            println!("{}", serde_json::to_string_pretty(&nodes)?);
        }
        NodeCommand::Stats { state, name } => {
            let toolchain = Toolchain::resolve(&state.state_dir, None).await?;
            let settings = toolchain.settings()?;
            let node = settings.node(&name)?;
            let stats = toolchain
                .validator_console_tool
                .health(
                    &OperationContext::for_node(Duration::from_secs(20), &node.name),
                    &toolchain.validator_console_endpoint(node),
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

fn layout(state_dir: &Path) -> Result<Layout> {
    let layout = Layout::new(crate::ton::toolchain::absolute_path(state_dir)?);
    layout.create_dirs()?;
    Ok(layout)
}
