use std::{fs, path::Path};

use anyhow::{Context, Result, ensure};
use serde::Serialize;

use crate::{
    cli::NodeCommand,
    storage::Layout,
    storage::{NodeRuntime, RuntimeState},
    storage::{NodeSettings, Settings},
    ton::toolchain::Toolchain,
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
        NodeCommand::Add {
            state,
            name,
            fullnode_only,
            liteserver,
        } => {
            let layout = layout(&state.state_dir)?;
            let mut settings = Settings::load_or_create(&layout.settings)?;
            let node = settings.node_mut(&name)?;
            node.enabled = true;
            node.validator = !fullnode_only;
            node.participate_in_elections = !fullnode_only;
            node.liteserver = liteserver;
            settings.save_atomic(&layout.settings)?;
            notify_live_launcher(&layout, &settings, &name, "start").await?;
            println!("enabled node `{name}`");
        }
        NodeCommand::Start { state, name } => {
            let layout = layout(&state.state_dir)?;
            let mut settings = Settings::load_or_create(&layout.settings)?;
            settings.node_mut(&name)?.enabled = true;
            settings.save_atomic(&layout.settings)?;
            notify_live_launcher(&layout, &settings, &name, "start").await?;
            println!("started node `{name}`");
        }
        NodeCommand::Stop { state, name } => {
            let layout = layout(&state.state_dir)?;
            let settings = Settings::load_or_create(&layout.settings)?;
            settings.node(&name)?;
            notify_live_launcher(&layout, &settings, &name, "stop").await?;
            println!("stopped node `{name}`; persistent state is intact");
        }
        NodeCommand::Remove {
            state,
            name,
            delete_state,
        } => {
            ensure!(name != "genesis", "the genesis node cannot be removed");
            let layout = layout(&state.state_dir)?;
            let mut settings = Settings::load_or_create(&layout.settings)?;
            settings.node(&name)?;
            notify_live_launcher(&layout, &settings, &name, "stop").await?;
            settings.node_mut(&name)?.enabled = false;
            settings.save_atomic(&layout.settings)?;
            if delete_state {
                let node_root = dunce::canonicalize(&layout.nodes)
                    .unwrap_or_else(|_| layout.nodes.clone())
                    .join(&name);
                ensure!(
                    node_root.starts_with(&layout.nodes),
                    "refusing to delete node state outside {}",
                    layout.nodes.display()
                );
                if node_root.is_dir() {
                    fs::remove_dir_all(&node_root).with_context(|| {
                        format!("failed to delete node state {}", node_root.display())
                    })?;
                }
            }
            println!(
                "disabled node `{name}`{}",
                if delete_state {
                    " and deleted its generated state"
                } else {
                    ""
                }
            );
        }
        NodeCommand::Stats { state, name } => {
            let toolchain = Toolchain::resolve(&state.state_dir, None).await?;
            let settings = toolchain.settings()?;
            let node = settings.node(&name)?;
            print!("{}", toolchain.validator_console(node, "getstats").await?);
        }
        NodeCommand::Console { state, name, args } => {
            let toolchain = Toolchain::resolve(&state.state_dir, None).await?;
            let settings = toolchain.settings()?;
            let node = settings.node(&name)?;
            print!(
                "{}",
                toolchain.validator_console(node, &args.join(" ")).await?
            );
        }
    }
    Ok(())
}

async fn notify_live_launcher(
    layout: &Layout,
    settings: &Settings,
    name: &str,
    action: &str,
) -> Result<()> {
    let runtime = RuntimeState::load(&layout.runtime)?;
    if !runtime.ready || runtime.launcher_pid.is_none() {
        return Ok(());
    }
    ensure!(
        settings.services.admin_http.enabled,
        "launcher is active but the administrative HTTP service is disabled"
    );
    let endpoint = format!(
        "http://{}:{}/v1/nodes/{name}/{action}",
        settings.services.admin_http.bind, settings.services.admin_http.port
    );
    let response = reqwest::Client::new()
        .post(&endpoint)
        .send()
        .await
        .with_context(|| format!("failed to contact active launcher at {endpoint}"))?;
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    ensure!(
        status.is_success(),
        "launcher rejected request: {status} {body}"
    );
    Ok(())
}

fn layout(state_dir: &Path) -> Result<Layout> {
    let layout = Layout::new(crate::ton::toolchain::absolute_path(state_dir)?);
    layout.create_dirs()?;
    Ok(layout)
}
