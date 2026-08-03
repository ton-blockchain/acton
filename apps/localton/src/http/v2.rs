//! Configures and runs the TON HTTP API V2 backend process.
//!
//! [`prepare_runtime`] creates a network-specific tonlib keystore and writes
//! `config_vars.yaml` with the global config, backend port, monitor port, worker
//! counts, and static content directory. [`start`] launches the executable under
//! [`ProcessRegistry`] and waits until `getMasterchainInfo` reports readiness.

use std::{
    env, fs,
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::{Context, Result, bail, ensure};
use serde_json::Value;
use sha2::{Digest, Sha256};
use tokio::{
    process::Command,
    time::{Instant, sleep},
};

use crate::{
    binaries::TonBinaries,
    runtime::{ManagedProcess, ProcessRegistry},
    storage::Layout,
    storage::{RuntimeState, ServiceRuntime},
    storage::{Settings, TonHttpApiSettings},
};

#[derive(Debug, Clone)]
pub struct Paths {
    pub executable: PathBuf,
    pub static_config: PathBuf,
    pub static_content: PathBuf,
}

impl Paths {
    pub fn new(layout: &Layout) -> Self {
        let install = layout.root.join("tools/ton-http-api-v2/install");
        Self {
            executable: install.join("bin/ton-http-api-v2"),
            static_config: install.join("config/static_config.yaml"),
            static_content: install.join("static"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Runtime {
    pub config_vars: PathBuf,
    pub static_config: PathBuf,
}

pub fn prepare_runtime(layout: &Layout, service: &TonHttpApiSettings) -> Result<Runtime> {
    ensure!(
        layout.global_config.is_file(),
        "TON HTTP API global config is missing: {}",
        layout.global_config.display()
    );

    let paths = Paths::new(layout);
    let static_config = service
        .static_config
        .clone()
        .unwrap_or_else(|| paths.static_config.clone());
    ensure!(
        static_config.is_file(),
        "TON HTTP API V2 static config is missing: {}; run `cargo xtask build-ton-http-api-v2 --state-dir {}`",
        static_config.display(),
        layout.root.display()
    );

    let runtime_dir = layout.root.join("services/ton-http-api-v2");
    let keystore = runtime_dir
        .join("keystore")
        .join(network_fingerprint(&layout.global_config)?);
    let config_vars = runtime_dir.join("config_vars.yaml");
    fs::create_dir_all(&keystore)
        .with_context(|| format!("failed to create {}", keystore.display()))?;

    let packaged_static_content = static_config
        .parent()
        .and_then(Path::parent)
        .map(|install| install.join("static"));
    let static_content = if paths.static_content.is_dir() {
        paths.static_content
    } else if packaged_static_content
        .as_ref()
        .is_some_and(|path| path.is_dir())
    {
        packaged_static_content.expect("packaged static content was checked")
    } else {
        let fallback = runtime_dir.join("static");
        fs::create_dir_all(&fallback)
            .with_context(|| format!("failed to create {}", fallback.display()))?;
        let not_found = fallback.join("404.json");
        if !not_found.is_file() {
            fs::write(&not_found, b"{\"ok\":false,\"error\":\"not found\"}\n")
                .with_context(|| format!("failed to write {}", not_found.display()))?;
        }
        fallback
    };

    let config = format!(
        concat!(
            "tonlib_config_path: {}\n",
            "tonlib_keystore_path: {}\n",
            "tonlib_boc_endpoints: []\n",
            "tonlib_threads: 4\n",
            "server_port: {}\n",
            "monitor_port: {}\n",
            "main_worker_threads: 4\n",
            "fs_worker_threads: 1\n",
            "http_worker_threads: 2\n",
            "log_level: warning\n",
            "log_path: \"@stdout\"\n",
            "system_log_level: warning\n",
            "system_log_path: \"@stdout\"\n",
            "jsonrpc_log_level: warning\n",
            "jsonrpc_log_path: \"@stdout\"\n",
            "log_format: json\n",
            "http_worker_user_agent: localton\n",
            "static_content_dir: {}\n",
            "max_stack_entry_depth: 256\n"
        ),
        yaml_path(&layout.global_config)?,
        yaml_path(&keystore)?,
        service.backend_port,
        service.monitor_port,
        yaml_path(&static_content)?,
    );
    fs::write(&config_vars, config)
        .with_context(|| format!("failed to write {}", config_vars.display()))?;

    Ok(Runtime {
        config_vars,
        static_config,
    })
}

pub async fn start(
    layout: &Layout,
    binaries: &TonBinaries,
    settings: &Settings,
    timeout: Duration,
    processes: &ProcessRegistry,
    runtime: &mut RuntimeState,
) -> Result<()> {
    if !settings.services.ton_http_api.enabled {
        return Ok(());
    }

    let service = &settings.services.ton_http_api;
    let installed = Paths::new(layout).executable;
    let executable = service
        .command
        .as_deref()
        .and_then(resolve_executable)
        .or_else(|| installed.is_file().then_some(installed))
        .or_else(|| binaries.optional_command("ton-http-api-v2"))
        .context(
            "TON HTTP API V2 is enabled, but its executable was not found; run `cargo xtask build-ton-http-api-v2`",
        )?;
    ensure!(
        executable.is_file(),
        "TON HTTP API V2 executable is missing: {}",
        executable.display()
    );

    let prepared = prepare_runtime(layout, service)?;
    let mut command = Command::new(executable);
    command
        .arg("--config")
        .arg(prepared.static_config)
        .arg("--config_vars")
        .arg(prepared.config_vars);
    let process = ManagedProcess::spawn(
        "ton-http-api-v2",
        command,
        &layout.logs.join("ton-http-api-v2.stdout.log"),
        &layout.logs.join("ton-http-api-v2.stderr.log"),
    )?;
    let pid = process.id();
    processes.insert(process).await?;
    wait_ready(service.backend_port, timeout, processes)
        .await
        .with_context(|| {
            format!(
                "TON HTTP API V2 failed readiness; inspect {}",
                layout.logs.join("ton-http-api-v2.stderr.log").display()
            )
        })?;
    runtime.services.insert(
        "ton_http_api".to_owned(),
        ServiceRuntime {
            running: true,
            pid,
            endpoint: Some(format!("http://127.0.0.1:{}", service.port)),
            last_error: None,
        },
    );
    Ok(())
}

async fn wait_ready(port: u16, timeout: Duration, processes: &ProcessRegistry) -> Result<()> {
    let url = format!("http://127.0.0.1:{port}/api/v2/getMasterchainInfo");
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
        .context("failed to build TON HTTP API V2 readiness client")?;
    let deadline = Instant::now() + timeout;

    loop {
        processes.ensure_alive().await?;
        let last_error = match client.get(&url).send().await {
            Ok(response) if response.status().is_success() => match response.bytes().await {
                Ok(body) => match serde_json::from_slice::<Value>(&body) {
                    Ok(value) if value.get("ok").and_then(Value::as_bool) == Some(true) => {
                        return Ok(());
                    }
                    Ok(value) => format!("API is not ready: {value}"),
                    Err(error) => format!("invalid readiness response: {error}"),
                },
                Err(error) => format!("failed to read readiness response: {error}"),
            },
            Ok(response) => format!("readiness returned HTTP {}", response.status()),
            Err(error) => error.to_string(),
        };
        if Instant::now() >= deadline {
            bail!("timed out waiting for {url}: {last_error}");
        }
        sleep(Duration::from_millis(250)).await;
    }
}

fn resolve_executable(command: &Path) -> Option<PathBuf> {
    if command.is_file() {
        return Some(command.to_owned());
    }
    if command.components().count() > 1 {
        return None;
    }
    env::var_os("PATH").and_then(|path| {
        env::split_paths(&path)
            .map(|directory| directory.join(command))
            .find(|candidate| candidate.is_file())
    })
}

fn network_fingerprint(global_config: &Path) -> Result<String> {
    let bytes = fs::read(global_config)
        .with_context(|| format!("failed to read {}", global_config.display()))?;
    let config: serde_json::Value = serde_json::from_slice(&bytes)
        .with_context(|| format!("invalid global config {}", global_config.display()))?;
    let genesis = config
        .pointer("/validator/init_block")
        .or_else(|| config.pointer("/validator/zero_state"))
        .context("global config has neither validator.init_block nor validator.zero_state")?;
    Ok(hex::encode(Sha256::digest(serde_json::to_vec(genesis)?)))
}

fn yaml_path(path: &Path) -> Result<String> {
    serde_json::to_string(&path.to_string_lossy().as_ref()).context("failed to quote YAML path")
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn runtime_config_uses_local_global_config_and_ports() {
        let temp = tempdir().unwrap();
        let layout = Layout::new(temp.path().join("state"));
        layout.create_dirs().unwrap();
        fs::write(
            &layout.global_config,
            serde_json::to_vec(&serde_json::json!({
                "validator": {
                    "init_block": {
                        "workchain": -1,
                        "shard": i64::MIN,
                        "seqno": 0,
                        "root_hash": "root",
                        "file_hash": "file"
                    }
                }
            }))
            .unwrap(),
        )
        .unwrap();
        let paths = Paths::new(&layout);
        fs::create_dir_all(paths.static_config.parent().unwrap()).unwrap();
        fs::create_dir_all(&paths.static_content).unwrap();
        fs::write(&paths.static_config, "components_manager: {}").unwrap();
        fs::write(paths.static_content.join("404.json"), "{}").unwrap();

        let service = TonHttpApiSettings {
            monitor_port: 18_006,
            ..TonHttpApiSettings::default()
        };
        let runtime = prepare_runtime(&layout, &service).unwrap();
        let config = fs::read_to_string(runtime.config_vars).unwrap();

        assert!(config.contains(&layout.global_config.display().to_string()));
        assert!(config.contains("server_port: 18005"));
        assert!(config.contains("monitor_port: 18006"));
        assert!(config.contains("/keystore/"));
        assert!(!config.contains("ton-http-api-v2/keystore\""));
        assert_eq!(runtime.static_config, paths.static_config);
    }

    #[test]
    fn runtime_uses_static_content_next_to_packaged_config() {
        let temp = tempdir().unwrap();
        let layout = Layout::new(temp.path().join("state"));
        layout.create_dirs().unwrap();
        fs::write(
            &layout.global_config,
            r#"{"validator":{"init_block":{"root_hash":"root"}}}"#,
        )
        .unwrap();

        let install = temp.path().join("opt/ton-http-api-v2");
        let static_config = install.join("config/static_config.yaml");
        let static_content = install.join("static");
        fs::create_dir_all(static_config.parent().unwrap()).unwrap();
        fs::create_dir_all(&static_content).unwrap();
        fs::write(&static_config, "components_manager: {}").unwrap();
        fs::write(static_content.join("openapi.json"), "{}").unwrap();

        let service = TonHttpApiSettings {
            static_config: Some(static_config),
            ..TonHttpApiSettings::default()
        };
        let runtime = prepare_runtime(&layout, &service).unwrap();
        let config = fs::read_to_string(runtime.config_vars).unwrap();

        assert!(config.contains(&static_content.display().to_string()));
    }

    #[test]
    fn tonlib_keystore_fingerprint_changes_with_genesis() {
        let temp = tempdir().unwrap();
        let config = temp.path().join("global.config.json");
        fs::write(
            &config,
            r#"{"validator":{"init_block":{"root_hash":"first"}}}"#,
        )
        .unwrap();
        let first = network_fingerprint(&config).unwrap();
        fs::write(
            &config,
            r#"{"validator":{"init_block":{"root_hash":"second"}}}"#,
        )
        .unwrap();
        let second = network_fingerprint(&config).unwrap();

        assert_eq!(first.len(), 64);
        assert_eq!(second.len(), 64);
        assert_ne!(first, second);
    }
}
