use acton_config::color::OwoColorize;
use anyhow::Context;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

pub async fn localnet_state_dump_cmd(
    path: PathBuf,
    force: bool,
    port: u16,
    auth_token: Option<String>,
) -> anyhow::Result<()> {
    let path = resolve_project_path(path);
    if path.exists() && !force {
        anyhow::bail!(
            "Output file {} already exists; pass {} to overwrite it",
            path.display().to_string().cyan(),
            "--force".yellow(),
        );
    }
    let json =
        super::get_localnet_control_bytes(port, auth_token, "acton_dumpState", &[], "Dump state")
            .await?;
    write_json_atomically(&path, &json)?;

    println!(
        "{} localnet state to {}",
        "Dumped".green().bold(),
        display_project_path(&path).dimmed(),
    );
    Ok(())
}

pub async fn localnet_state_load_cmd(
    path: PathBuf,
    port: u16,
    auth_token: Option<String>,
) -> anyhow::Result<()> {
    let path = resolve_project_path(path);
    let json = fs::read(&path).with_context(|| {
        format!(
            "Failed to read localnet state file {}",
            path.display().to_string().cyan()
        )
    })?;
    super::post_localnet_control_bytes(
        port,
        auth_token,
        "acton_loadState",
        &[],
        json,
        "Load state",
    )
    .await?;

    println!(
        "{} localnet state from {}",
        "Loaded".green().bold(),
        display_project_path(&path).dimmed(),
    );
    Ok(())
}

pub async fn localnet_checkpoint_create_cmd(
    name: &str,
    force: bool,
    port: u16,
    auth_token: Option<String>,
) -> anyhow::Result<()> {
    let name = normalize_checkpoint_name(name)?;

    super::post_localnet_control(
        port,
        auth_token,
        "acton_createCheckpoint",
        serde_json::json!({
            "name": &name,
            "force": force,
        }),
        "Create checkpoint",
    )
    .await?;

    println!(
        "{} localnet checkpoint {}",
        "Created".green().bold(),
        name.cyan(),
    );
    Ok(())
}

pub async fn localnet_checkpoint_list_cmd(
    port: u16,
    auth_token: Option<String>,
) -> anyhow::Result<()> {
    let result = super::get_localnet_control(
        port,
        auth_token,
        "acton_listCheckpoints",
        "List checkpoints",
    )
    .await?;
    let checkpoints = result
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|checkpoint| {
            Some((
                checkpoint.get("name")?.as_str()?.to_owned(),
                checkpoint.get("block_seqno")?.as_u64()?,
            ))
        })
        .collect::<Vec<_>>();
    if checkpoints.is_empty() {
        println!("No localnet checkpoints found");
        return Ok(());
    }

    println!("{}", "Localnet checkpoints:".white().bold());
    for (name, block_seqno) in checkpoints {
        let block = format!("(block {block_seqno})");
        println!("  {} {}", name.cyan(), block.dimmed());
    }
    Ok(())
}

pub async fn localnet_checkpoint_restore_cmd(
    name: &str,
    port: u16,
    auth_token: Option<String>,
) -> anyhow::Result<()> {
    let name = normalize_checkpoint_name(name)?;

    super::post_localnet_control(
        port,
        auth_token,
        "acton_restoreCheckpoint",
        serde_json::json!({ "name": &name }),
        "Restore checkpoint",
    )
    .await?;

    println!(
        "{} localnet checkpoint {}",
        "Restored".green().bold(),
        name.cyan(),
    );
    Ok(())
}

pub async fn localnet_checkpoint_delete_cmd(
    name: &str,
    port: u16,
    auth_token: Option<String>,
) -> anyhow::Result<()> {
    let name = normalize_checkpoint_name(name)?;
    super::post_localnet_control(
        port,
        auth_token,
        "acton_deleteCheckpoint",
        serde_json::json!({ "name": &name }),
        "Delete checkpoint",
    )
    .await?;

    println!(
        "{} localnet checkpoint {}",
        "Deleted".green().bold(),
        name.cyan(),
    );
    Ok(())
}

pub async fn localnet_checkpoint_clear_cmd(
    port: u16,
    auth_token: Option<String>,
) -> anyhow::Result<()> {
    let result = super::post_localnet_control(
        port,
        auth_token,
        "acton_clearCheckpoints",
        serde_json::json!({}),
        "Clear checkpoints",
    )
    .await?;
    let deleted = result
        .get("deleted")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or_default();
    println!(
        "{} {deleted} localnet checkpoint{}",
        "Deleted".green().bold(),
        if deleted == 1 { "" } else { "s" },
    );
    Ok(())
}

pub async fn localnet_checkpoint_export_cmd(
    name: &str,
    out: PathBuf,
    force: bool,
    port: u16,
    auth_token: Option<String>,
) -> anyhow::Result<()> {
    let name = normalize_checkpoint_name(name)?;
    let out = resolve_project_path(out);
    if out.exists() && !force {
        anyhow::bail!(
            "Output file {} already exists; pass {} to overwrite it",
            out.display().to_string().cyan(),
            "--force".yellow(),
        );
    }
    let json = super::get_localnet_control_bytes(
        port,
        auth_token,
        "acton_exportCheckpoint",
        &[("name", name.as_str())],
        "Export checkpoint",
    )
    .await?;
    write_json_atomically(&out, &json)?;

    println!(
        "{} localnet checkpoint {} to {}",
        "Exported".green().bold(),
        name.cyan(),
        display_project_path(&out).dimmed(),
    );
    Ok(())
}

pub async fn localnet_checkpoint_import_cmd(
    path: PathBuf,
    name: Option<String>,
    force: bool,
    port: u16,
    auth_token: Option<String>,
) -> anyhow::Result<()> {
    let path = resolve_project_path(path);
    let json = fs::read(&path).with_context(|| {
        format!(
            "Failed to read checkpoint file {}",
            path.display().to_string().cyan()
        )
    })?;
    let name = match name {
        Some(name) => normalize_checkpoint_name(&name)?,
        None => checkpoint_name_from_path(&path)?,
    };
    let force = force.to_string();
    super::post_localnet_control_bytes(
        port,
        auth_token,
        "acton_importCheckpoint",
        &[("name", name.as_str()), ("force", force.as_str())],
        json,
        "Import checkpoint",
    )
    .await?;

    println!(
        "{} localnet checkpoint {} from {}",
        "Imported".green().bold(),
        name.cyan(),
        display_project_path(&path).dimmed(),
    );
    Ok(())
}

fn resolve_project_path(path: PathBuf) -> PathBuf {
    if path.is_absolute() {
        path
    } else {
        acton_config::config::project_root().join(path)
    }
}

fn write_json_atomically(path: &Path, json: &[u8]) -> anyhow::Result<()> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;

    let mut temp = tempfile::NamedTempFile::new_in(parent)?;
    temp.write_all(json)?;
    temp.as_file_mut().flush()?;
    temp.as_file().sync_all()?;
    temp.persist(path).map_err(|error| error.error)?;
    Ok(())
}

fn normalize_checkpoint_name(name: &str) -> anyhow::Result<String> {
    let name = name.trim();
    if name.is_empty() {
        anyhow::bail!("Localnet checkpoint name cannot be empty");
    }
    if name == "." || name == ".." {
        anyhow::bail!("Localnet checkpoint name cannot be {}", name.cyan());
    }
    if !name
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
    {
        anyhow::bail!(
            "Invalid localnet checkpoint name {}; use only letters, numbers, '.', '_' and '-'",
            name.cyan(),
        );
    }
    Ok(name.to_owned())
}

fn checkpoint_name_from_path(path: &Path) -> anyhow::Result<String> {
    let stem = path
        .file_stem()
        .and_then(|name| name.to_str())
        .with_context(|| {
            format!(
                "Cannot infer localnet checkpoint name from file {}; pass {}",
                path.display().to_string().cyan(),
                "--name".yellow(),
            )
        })?;
    normalize_checkpoint_name(stem)
}

fn display_project_path(path: &Path) -> String {
    path.strip_prefix(acton_config::config::project_root())
        .unwrap_or(path)
        .display()
        .to_string()
}
