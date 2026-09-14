use acton_config::color::OwoColorize;
use anyhow::Context;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Saved JSON snapshots use stable IDs; import creates a new snapshot and restore is explicit.
#[derive(clap::Subcommand, Clone)]
pub enum SnapshotCommand {
    #[command(about = "Save the current network state as a persistent JSON snapshot")]
    Create { name: Option<String> },
    #[command(about = "List saved snapshots")]
    List,
    #[command(about = "Restore the network state from a saved snapshot")]
    Restore { id: String },
    #[command(about = "Delete a saved snapshot")]
    Delete { id: String },
    #[command(about = "Export a saved snapshot to a JSON file")]
    Export {
        id: String,
        #[arg(long)]
        out: PathBuf,
        #[arg(long, help = "Overwrite the output file")]
        force: bool,
    },
    #[command(about = "Import a JSON snapshot without restoring it")]
    Import {
        path: PathBuf,
        #[arg(long)]
        name: Option<String>,
    },
}

/// Uses the node control API so CLI and Studio share snapshot validation and ownership.
pub async fn simulated_localnet_snapshot_cmd(
    command: SnapshotCommand,
    port: u16,
    auth_token: Option<String>,
    json: bool,
) -> anyhow::Result<()> {
    let result = match command {
        SnapshotCommand::Create { name } => {
            super::post_localnet_control(
                port,
                auth_token,
                "acton_createSnapshot",
                serde_json::json!({ "name": name }),
                "Create snapshot",
            )
            .await?
        }
        SnapshotCommand::List => {
            super::get_localnet_control(port, auth_token, "acton_listSnapshots", "List snapshots")
                .await?
        }
        SnapshotCommand::Restore { id } => {
            super::post_localnet_control(
                port,
                auth_token,
                "acton_restoreSnapshot",
                serde_json::json!({ "id": id }),
                "Restore snapshot",
            )
            .await?
        }
        SnapshotCommand::Delete { id } => {
            super::post_localnet_control(
                port,
                auth_token,
                "acton_deleteSnapshot",
                serde_json::json!({ "id": id }),
                "Delete snapshot",
            )
            .await?;

            serde_json::json!({ "deleted": id })
        }
        SnapshotCommand::Export { id, out, force } => {
            let out = resolve_project_path(out);
            let bytes = super::get_localnet_control_bytes(
                port,
                auth_token,
                "acton_exportSnapshot",
                &[("id", id.as_str())],
                "Export snapshot",
            )
            .await?;

            write_json_atomically(&out, &bytes, force)?;

            serde_json::json!({ "exported": id, "path": display_project_path(&out) })
        }
        SnapshotCommand::Import { path, name } => {
            let path = resolve_project_path(path);
            let bytes = fs::read(&path)
                .with_context(|| format!("Cannot read snapshot {}", path.display()))?;
            let query = name
                .as_deref()
                .map(|name| vec![("name", name)])
                .unwrap_or_default();

            super::post_localnet_control_bytes(
                port,
                auth_token,
                "acton_importSnapshot",
                &query,
                bytes,
                "Import snapshot",
            )
            .await?
        }
    };

    if json {
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else if let Some(snapshots) = result.as_array() {
        if snapshots.is_empty() {
            println!("No snapshots yet");
        }

        for snapshot in snapshots {
            print_snapshot(snapshot);
        }
    } else if result.get("id").is_some() {
        print_snapshot(&result);
    } else {
        println!("{}", serde_json::to_string_pretty(&result)?);
    }

    Ok(())
}

fn print_snapshot(snapshot: &serde_json::Value) {
    println!(
        "{}  {}  (block {}, {} bytes)",
        snapshot["id"].as_str().unwrap_or_default().cyan(),
        snapshot["name"].as_str().unwrap_or("Snapshot"),
        snapshot["block_seqno"],
        snapshot["size_bytes"]
    );
}

fn resolve_project_path(path: PathBuf) -> PathBuf {
    if path.is_absolute() {
        path
    } else {
        acton_config::config::project_root().join(path)
    }
}

fn write_json_atomically(path: &Path, json: &[u8], force: bool) -> anyhow::Result<()> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;

    let mut temp = tempfile::NamedTempFile::new_in(parent)?;
    temp.write_all(json)?;
    temp.as_file_mut().flush()?;
    temp.as_file().sync_all()?;

    if force {
        temp.persist(path)
    } else {
        temp.persist_noclobber(path)
    }
    .map_err(|error| error.error)
    .with_context(|| {
        format!(
            "Cannot export snapshot to {}; use --force to replace an existing file",
            path.display()
        )
    })?;

    Ok(())
}

fn display_project_path(path: &Path) -> String {
    path.strip_prefix(acton_config::config::project_root())
        .unwrap_or(path)
        .display()
        .to_string()
}
