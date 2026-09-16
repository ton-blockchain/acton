//! Reconcile named private overlays through the validator engine's control API.

use std::{collections::BTreeMap, process::Stdio};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};
use tokio::{io::AsyncWriteExt, process::Command, time::timeout};

use super::{COMPOSE_NODE_COMMAND_TIMEOUT, DockerNetwork, LOCALTON_STATE_DIR};
use crate::{Error, Node, OverlayConfig};

const FAILURE_CODE: &str = "environment_overlays_failed";
const CONSOLE: &str = "/opt/ton/validator-engine-console";
const CLIENT_KEY: &str = "/var/lib/localton/node/certs/client";
const SERVER_KEY: &str = "/var/lib/localton/node/certs/server.pub";

// Only these constant scripts reach a shell. Names, addresses and JSON never
// become shell source; the latter is streamed through stdin into a unique file.
const READ_STATE: &str = r#"
set -eu
printf '{"engine":'
cat /var/lib/localton/node/db/config.json
printf ',"overlays":'
if [ -f /var/lib/localton/node/db/custom-overlays.json ]; then
  cat /var/lib/localton/node/db/custom-overlays.json
else
  printf '{"overlays":[]}'
fi
printf '}'
"#;
const ADD_OVERLAY: &str = r#"
set -eu
file=$(mktemp /tmp/acton-overlay.XXXXXX)
trap 'rm -f "$file"' EXIT HUP INT TERM
cat > "$file"
/opt/ton/validator-engine-console -t 10 -v 0 -k /var/lib/localton/node/certs/client -p /var/lib/localton/node/certs/server.pub -a "$1" -rc "add-custom-overlay $file"
"#;

#[derive(Clone, Debug)]
struct Target {
    id: String,
    service: String,
    console_port: u16,
    stopped: bool,
}

#[derive(Debug, Deserialize)]
struct EngineConfig {
    fullnode: String,
}

#[derive(Debug, Deserialize)]
struct EngineState {
    engine: EngineConfig,
    overlays: TonOverlays,
}

#[derive(Debug, Deserialize)]
struct TonOverlays {
    overlays: Vec<TonOverlay>,
}

// Keep TON's spelling at the adapter boundary. Older releases omit the query
// flags; false defaults preserve their broadcast-only behavior and idempotence.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct TonOverlay {
    #[serde(rename = "@type")]
    constructor: OverlayConstructor,
    name: String,
    nodes: Vec<TonOverlayNode>,
    #[serde(default)]
    sender_shards: Vec<serde_json::Value>,
    #[serde(default)]
    skip_public_msg_send: bool,
    #[serde(default)]
    use_quic: bool,
    #[serde(default)]
    send_queries: bool,
    #[serde(flatten)]
    extra: BTreeMap<String, serde_json::Value>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct TonOverlayNode {
    #[serde(rename = "@type")]
    constructor: OverlayNodeConstructor,
    adnl_id: String,
    msg_sender: bool,
    msg_sender_priority: i32,
    block_sender: bool,
    #[serde(default)]
    accept_queries: bool,
    #[serde(flatten)]
    extra: BTreeMap<String, serde_json::Value>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
enum OverlayConstructor {
    #[serde(rename = "engine.validator.customOverlay")]
    Overlay,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
enum OverlayNodeConstructor {
    #[serde(rename = "engine.validator.customOverlayNode")]
    Node,
}

#[derive(Debug, Default)]
struct OverlayChanges {
    remove: Vec<String>,
    add: Vec<TonOverlay>,
}

impl OverlayChanges {
    const fn is_empty(&self) -> bool {
        self.remove.is_empty() && self.add.is_empty()
    }
}

impl DockerNetwork {
    /// Replaces the complete custom-overlay set on each active node. Stopped
    /// members retain their identities and are reconciled when they next start.
    pub(crate) async fn configure_overlays(
        &self,
        nodes: &[Node],
        config: &OverlayConfig,
    ) -> Result<(), Error> {
        config.validate(nodes)?;
        let targets = targets(nodes)?;
        let mut identities = BTreeMap::new();
        let mut snapshots = Vec::new();

        // Capture all identities and original settings before changing anything.
        // The read-only volume path also works for explicitly stopped members.
        for target in targets {
            let state = self.overlay_state(&target).await?;
            identities.insert(target.id.clone(), fullnode_id(&state.engine.fullnode)?);
            if !target.stopped {
                snapshots.push((target, state.overlays.overlays));
            }
        }

        let desired = desired_overlays(config, &identities)?;
        let mut plans = Vec::new();
        for (target, current) in &snapshots {
            let own = config
                .overlays
                .iter()
                .zip(&desired)
                .filter(|(definition, _)| definition.nodes.contains(&target.id))
                .map(|(_, overlay)| overlay.clone())
                .collect::<Vec<_>>();
            plans.push(overlay_changes(current, &own)?);
        }

        // An interrupted first application must be reconciled even if the saved
        // topology is still empty. Read/preflight errors never claim ownership.
        let marker = self.compose_file.with_file_name("overlays-managed");
        let already_managed = tokio::fs::try_exists(&marker)
            .await
            .map_err(|error| failure(format!("Could not inspect overlay management: {error}")))?;
        tokio::fs::write(&marker, b"")
            .await
            .map_err(|error| failure(format!("Could not record overlay management: {error}")))?;

        for ((target, _), changes) in snapshots.iter().zip(&plans) {
            if changes.is_empty() {
                continue;
            }
            if let Err(error) = self.apply_overlay_changes(target, changes).await {
                // A console may report failure after its engine has persisted
                // the change, so re-read actual state rather than reversing a
                // presumed prefix of completed commands.
                let mut failures = Vec::new();
                for (original_target, original) in &snapshots {
                    let restored = async {
                        let current = self.overlay_state(original_target).await?;
                        let undo = overlay_changes(&current.overlays.overlays, original)?;
                        self.apply_overlay_changes(original_target, &undo).await
                    }
                    .await;
                    if let Err(restore) = restored {
                        failures.push(format!("{}: {restore}", original_target.id));
                    }
                }
                if failures.is_empty() && !already_managed {
                    // A failed first update did not take ownership of the
                    // restored, possibly externally configured overlay set.
                    if let Err(remove) = tokio::fs::remove_file(&marker).await {
                        failures.push(format!(
                            "Could not remove overlay management marker: {remove}"
                        ));
                    }
                }
                return if failures.is_empty() {
                    Err(error)
                } else {
                    let recovery = self.compose_file.with_file_name("overlays-recovery");
                    if let Err(mark) = tokio::fs::write(&recovery, b"").await {
                        failures.push(format!("Could not record pending recovery: {mark}"));
                    }
                    Err(failure(format!(
                        "{error}. Restoring the previous overlays also failed: {}",
                        failures.join("; ")
                    )))
                };
            }
        }
        Ok(())
    }

    async fn overlay_state(&self, target: &Target) -> Result<EngineState, Error> {
        let mut command = if target.stopped {
            let mut command = self.docker_command();
            command
                .args(["run", "--rm", "--network", "none", "--volume"])
                .arg(format!(
                    "{}_{}-state:{LOCALTON_STATE_DIR}:ro",
                    self.project_name, target.service
                ))
                .args(["--entrypoint", "/bin/sh", &self.image, "-c", READ_STATE]);
            command
        } else {
            let mut command = self.compose_command();
            command.args(["exec", "-T", &target.service, "sh", "-c", READ_STATE]);
            command
        };
        command.kill_on_drop(true);
        let output = self
            .command_output(
                command,
                &format!("read overlay state on {}", target.id),
                FAILURE_CODE,
                COMPOSE_NODE_COMMAND_TIMEOUT,
            )
            .await?;
        serde_json::from_slice(&output.stdout)
            .map_err(|error| failure(format!("Invalid overlay state on {}: {error}", target.id)))
    }

    async fn apply_overlay_changes(
        &self,
        target: &Target,
        changes: &OverlayChanges,
    ) -> Result<(), Error> {
        for name in &changes.remove {
            let output = self
                .overlay_console(target, &format!("del-custom-overlay {name}"))
                .await?;
            console_success(&output, &target.id)?;
        }
        for overlay in &changes.add {
            let input = serde_json::to_vec(overlay).map_err(failure)?;
            let mut command = self.compose_command();
            command
                .args(["exec", "-T", &target.service, "sh", "-c", ADD_OVERLAY, "sh"])
                .arg(format!("127.0.0.1:{}", target.console_port));
            self.add_overlay_input(command, &target.id, &input).await?;
        }
        Ok(())
    }

    async fn overlay_console(&self, target: &Target, query: &str) -> Result<String, Error> {
        let mut command = self.compose_command();
        command
            .args([
                "exec",
                "-T",
                &target.service,
                CONSOLE,
                "-t",
                "10",
                "-v",
                "0",
                "-k",
                CLIENT_KEY,
                "-p",
                SERVER_KEY,
                "-a",
            ])
            .arg(format!("127.0.0.1:{}", target.console_port))
            .args(["-rc", query]);
        let output = self
            .command_output(
                command,
                &format!("configure overlays on {}", target.id),
                FAILURE_CODE,
                COMPOSE_NODE_COMMAND_TIMEOUT,
            )
            .await?;
        Ok(output_text(&output))
    }

    async fn add_overlay_input(
        &self,
        mut command: Command,
        id: &str,
        input: &[u8],
    ) -> Result<(), Error> {
        command
            .kill_on_drop(true)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        timeout(COMPOSE_NODE_COMMAND_TIMEOUT, async {
            let mut child = command.spawn().map_err(failure)?;
            let mut stdin = child
                .stdin
                .take()
                .ok_or_else(|| failure("Missing command stdin"))?;
            let write = async {
                let result = stdin.write_all(input).await;
                drop(stdin);
                result
            };
            let (written, output) = tokio::join!(write, child.wait_with_output());
            let output = output.map_err(failure)?;
            if !output.status.success() {
                return Err(failure(format!(
                    "Could not configure overlays on {id} ({}): {}",
                    output.status,
                    output_text(&output).trim()
                )));
            }
            written.map_err(failure)?;
            console_success(&output_text(&output), id)
        })
        .await
        .map_err(|_| failure(format!("Timed out configuring overlays on {id}")))?
    }
}

fn targets(nodes: &[Node]) -> Result<Vec<Target>, Error> {
    let mut result = vec![Target {
        id: "genesis".into(),
        service: "localton".into(),
        console_port: 4441,
        stopped: false,
    }];
    for node in nodes {
        result.push(Target {
            id: node.id.clone(),
            service: node.id.clone(),
            console_port: node
                .port_base
                .checked_add(1)
                .ok_or_else(|| failure(format!("Invalid console port for node {}", node.id)))?,
            stopped: node.stopped,
        });
    }
    Ok(result)
}

fn fullnode_id(value: &str) -> Result<String, Error> {
    let bytes = STANDARD
        .decode(value)
        .map_err(|_| failure("Validator fullnode identity is not a base64 ADNL ID"))?;
    if bytes.len() != 32 || bytes.iter().all(|byte| *byte == 0) {
        return Err(failure(
            "Validator fullnode identity must be a nonzero 256-bit ADNL ID",
        ));
    }
    Ok(STANDARD.encode(bytes))
}

fn desired_overlays(
    config: &OverlayConfig,
    identities: &BTreeMap<String, String>,
) -> Result<Vec<TonOverlay>, Error> {
    config
        .overlays
        .iter()
        .map(|overlay| {
            let mut nodes = overlay
                .nodes
                .iter()
                .map(|id| {
                    Ok(TonOverlayNode {
                        constructor: OverlayNodeConstructor::Node,
                        adnl_id: identities
                            .get(id)
                            .ok_or_else(|| failure(format!("No ADNL identity for node {id}")))?
                            .clone(),
                        msg_sender: true,
                        msg_sender_priority: 0,
                        block_sender: true,
                        accept_queries: false,
                        extra: BTreeMap::new(),
                    })
                })
                .collect::<Result<Vec<_>, Error>>()?;
            nodes.sort_by(|a, b| a.adnl_id.cmp(&b.adnl_id));
            if nodes
                .windows(2)
                .any(|pair| pair[0].adnl_id == pair[1].adnl_id)
            {
                return Err(failure(format!(
                    "Overlay {:?} contains nodes with the same ADNL identity",
                    overlay.name
                )));
            }
            Ok(TonOverlay {
                constructor: OverlayConstructor::Overlay,
                name: overlay.name.clone(),
                nodes,
                sender_shards: Vec::new(),
                skip_public_msg_send: false,
                use_quic: false,
                send_queries: false,
                extra: BTreeMap::new(),
            })
        })
        .collect()
}

fn overlay_changes(
    current: &[TonOverlay],
    desired: &[TonOverlay],
) -> Result<OverlayChanges, Error> {
    let normalize = |overlays: &[TonOverlay]| {
        let mut result = BTreeMap::new();
        for overlay in overlays {
            let mut overlay = overlay.clone();
            overlay.nodes.sort_by(|a, b| a.adnl_id.cmp(&b.adnl_id));
            if result.insert(overlay.name.clone(), overlay).is_some() {
                return Err(failure(
                    "Engine configuration contains duplicate overlay names",
                ));
            }
        }
        Ok(result)
    };
    let current = normalize(current)?;
    let desired = normalize(desired)?;
    let mut changes = OverlayChanges::default();
    for (name, overlay) in &current {
        if desired.get(name) != Some(overlay) {
            // The console tokenizer does not support quoted tokens. Refuse an
            // unaddressable preexisting name before any node has been changed.
            if name.is_empty() || name.chars().any(char::is_whitespace) {
                return Err(failure(format!(
                    "Existing overlay name {name:?} cannot be addressed by validator-engine-console"
                )));
            }
            changes.remove.push(name.clone());
        }
    }
    for (name, overlay) in desired {
        if current.get(&name) != Some(&overlay) {
            changes.add.push(overlay);
        }
    }
    Ok(changes)
}

fn output_text(output: &std::process::Output) -> String {
    format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn console_success(output: &str, id: &str) -> Result<(), Error> {
    let lower = output.to_ascii_lowercase();
    if !output.lines().any(|line| line.trim() == "success")
        || lower.contains("failed")
        || lower.contains("error")
    {
        return Err(failure(format!(
            "Validator console did not confirm overlay configuration on {id}: {}",
            output.trim()
        )));
    }
    Ok(())
}

fn failure(message: impl std::fmt::Display) -> Error {
    Error::Internal {
        code: FAILURE_CODE,
        message: message.to_string(),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests;
