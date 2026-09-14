use super::*;
use crate::{NetworkConfig, PrivateOverlay};

fn topology(name: &str, nodes: &[&str]) -> OverlayConfig {
    OverlayConfig {
        overlays: vec![PrivateOverlay {
            name: name.into(),
            nodes: nodes.iter().map(|id| (*id).into()).collect(),
        }],
    }
}

fn identities() -> BTreeMap<String, String> {
    BTreeMap::from([
        ("genesis".into(), STANDARD.encode([1; 32])),
        ("node-1".into(), STANDARD.encode([2; 32])),
        ("node-2".into(), STANDARD.encode([3; 32])),
    ])
}

#[test]
fn ton_json_uses_full_adnl_membership_and_broadcast_permissions() {
    let overlays = desired_overlays(
        &topology("validators", &["node-1", "genesis"]),
        &identities(),
    )
    .unwrap();
    let json = serde_json::to_value(&overlays[0]).unwrap();
    assert_eq!(json["@type"], "engine.validator.customOverlay");
    assert_eq!(json["name"], "validators");
    assert_eq!(json["sender_shards"], serde_json::json!([]));
    assert_eq!(json["skip_public_msg_send"], false);
    assert_eq!(json["use_quic"], false);
    assert_eq!(json["send_queries"], false);
    assert_eq!(
        json["nodes"],
        serde_json::json!([
            {"@type": "engine.validator.customOverlayNode", "adnl_id": STANDARD.encode([1; 32]),
             "msg_sender": true, "msg_sender_priority": 0, "block_sender": true,
             "accept_queries": false},
            {"@type": "engine.validator.customOverlayNode", "adnl_id": STANDARD.encode([2; 32]),
             "msg_sender": true, "msg_sender_priority": 0, "block_sender": true,
             "accept_queries": false}
        ])
    );
    let decoded: TonOverlay = serde_json::from_value(json).unwrap();
    assert_eq!(decoded, overlays[0]);
}

#[test]
fn reconciliation_ignores_member_order_but_replaces_changed_membership() {
    let current =
        desired_overlays(&topology("peers", &["genesis", "node-1"]), &identities()).unwrap();
    let mut reordered = current.clone();
    reordered[0].nodes.reverse();
    assert!(overlay_changes(&current, &reordered).unwrap().is_empty());

    let next = desired_overlays(&topology("peers", &["genesis", "node-2"]), &identities()).unwrap();
    let changes = overlay_changes(&current, &next).unwrap();
    assert_eq!(changes.remove, ["peers"]);
    assert_eq!(changes.add, next);
    let removed = overlay_changes(&current, &[]).unwrap();
    assert_eq!(removed.remove, ["peers"]);
    assert!(removed.add.is_empty());
}

#[test]
fn older_ton_config_defaults_query_flags_without_churn() {
    let current =
        desired_overlays(&topology("peers", &["genesis", "node-1"]), &identities()).unwrap();
    let mut json = serde_json::to_value(&current[0]).unwrap();
    json.as_object_mut().unwrap().remove("send_queries");
    for node in json["nodes"].as_array_mut().unwrap() {
        node.as_object_mut().unwrap().remove("accept_queries");
    }
    let stored: TonOverlay = serde_json::from_value(json).unwrap();
    assert!(overlay_changes(&[stored], &current).unwrap().is_empty());
}

#[test]
fn snapshot_preserves_unknown_engine_settings_for_rollback() {
    let current =
        desired_overlays(&topology("existing", &["genesis", "node-1"]), &identities()).unwrap();
    let mut json = serde_json::to_value(&current[0]).unwrap();
    json["future_setting"] = serde_json::json!({"limit": 7});
    json["nodes"][0]["future_node_setting"] = serde_json::json!(42);
    json["skip_public_msg_send"] = serde_json::json!(true);
    let original: TonOverlay = serde_json::from_value(json.clone()).unwrap();
    let restore = overlay_changes(&current, &[original]).unwrap();
    assert_eq!(restore.remove, ["existing"]);
    assert_eq!(serde_json::to_value(&restore.add[0]).unwrap(), json);
}

#[test]
fn preflight_rejects_unaddressable_names_and_duplicate_adnl_ids() {
    let mut current =
        desired_overlays(&topology("old", &["genesis", "node-1"]), &identities()).unwrap();
    current[0].name = "old name".into();
    assert!(overlay_changes(&current, &[]).is_err());
    let mut ids = identities();
    ids.insert("node-1".into(), ids["genesis"].clone());
    assert!(desired_overlays(&topology("peers", &["genesis", "node-1"]), &ids).is_err());
    assert!(fullnode_id(&STANDARD.encode([0; 32])).is_err());
    assert!(fullnode_id(&STANDARD.encode([1; 31])).is_err());
}

#[test]
fn console_requires_semantic_success_even_after_zero_exit_status() {
    assert!(console_success("conn ready\nsuccess\n", "genesis").is_ok());
    assert!(console_success("conn ready\n", "genesis").is_err());
    assert!(console_success("Failed: not started\n", "genesis").is_err());
    assert!(console_success("success\n[Error : 651 : not authorized]\n", "genesis").is_err());
}

#[tokio::test]
async fn add_reports_child_failure_when_stdin_closes_early() {
    let dir = tempfile::tempdir().unwrap();
    let driver = DockerNetwork {
        compose_file: dir.path().join("compose.yaml"),
        compose_config: NetworkConfig {
            port_base: 0,
            ports: None,
            block_time_ms: None,
            election_time_seconds: None,
            imported_account_bocs: vec![],
        },
        docker_target: super::super::DockerTarget::Context("unused".into()),
        isolated_docker_config_dir: None,
        image: "unused".into(),
        project_name: "unused".into(),
        startup_log_file: dir.path().join("startup.log"),
    };
    let mut command = Command::new("sh");
    command.args([
        "-c",
        "exec 0<&-; echo 'overlay operation unavailable' >&2; exit 1",
    ]);
    let error = driver
        .add_overlay_input(command, "genesis", &vec![0; 1024 * 1024])
        .await
        .unwrap_err();
    assert!(error.to_string().contains("overlay operation unavailable"));
}

#[tokio::test]
#[ignore = "requires Docker, Localton image, and free ports 29750-29754"]
async fn private_overlays_live() -> anyhow::Result<()> {
    use crate::docker::test_support::completed;
    use crate::{CreateNetwork, Runtime, catalog, runtime::Action};
    use anyhow::ensure;

    let directory = tempfile::tempdir_in("/tmp")?;
    let location = catalog::create(
        directory.path(),
        CreateNetwork {
            name: "private-overlays-regression".into(),
            port_base: Some(29750),
            block_time_ms: Some(1000),
            election_time_seconds: Some(3600),
            ..Default::default()
        },
    )
    .await?;
    let runtime = Runtime::open(&location.path).await?;
    let driver =
        DockerNetwork::materialize(&location.path, directory.path(), &location.network, false)
            .await?;
    eprintln!("Overlay regression deployment: {}", driver.project_name);
    let result: anyhow::Result<()> = async {
        completed(&runtime, Action::Start).await?;
        for name in ["first", "second"] {
            completed(
                &runtime,
                Action::AddNode {
                    name: name.into(),
                    validator: false,
                },
            )
            .await?;
        }
        let nodes = runtime.get().await.nodes;
        let targets = targets(&nodes)?;
        let initial = topology("private", &["genesis", "node-1"]);
        completed(&runtime, Action::ConfigureOverlays(initial.clone())).await?;
        ensure!(location.path.join("overlays-managed").exists());
        assert_membership(&driver, &targets, &[true, true, false]).await?;

        for target in &targets[..2] {
            let output = driver
                .overlay_console(target, "show-custom-overlays")
                .await?;
            ensure!(output.contains("Overlay \"private\": 2 nodes"), "{output}");
        }
        let genesis_id = driver.overlay_state(&targets[0]).await?.engine.fullnode;
        wait_private_broadcasts(&driver, &targets[1], &genesis_id).await?;
        let before = overlay_mtime(&driver, &targets[0]).await?;
        driver.configure_overlays(&nodes, &initial).await?;
        ensure!(
            before == overlay_mtime(&driver, &targets[0]).await?,
            "Unchanged overlays rewrote engine config"
        );

        // A missing client-side public key on the second participant makes its
        // command fail after genesis has changed. Exact original membership and
        // overlay names must be restored across the deployment.
        move_server_key(&driver, "node-1", true).await?;
        let failed = driver
            .configure_overlays(&nodes, &topology("replacement", &["genesis", "node-1"]))
            .await;
        move_server_key(&driver, "node-1", false).await?;
        ensure!(failed.is_err(), "Expected injected console failure");
        assert_membership(&driver, &targets, &[true, true, false]).await?;
        for target in &targets[..2] {
            let current = driver.overlay_state(target).await?;
            ensure!(
                current.overlays.overlays[0].name == "private",
                "Original overlay was not restored"
            );
        }

        completed(&runtime, Action::Stop).await?;
        completed(&runtime, Action::Start).await?;
        assert_membership(&driver, &targets, &[true, true, false]).await?;

        completed(
            &runtime,
            Action::ConfigureOverlays(topology("private", &["genesis", "node-2"])),
        )
        .await?;
        assert_membership(&driver, &targets, &[true, false, true]).await?;

        completed(
            &runtime,
            Action::ConfigureOverlays(OverlayConfig::default()),
        )
        .await?;
        assert_membership(&driver, &targets, &[false, false, false]).await?;
        Ok(())
    }
    .await;

    if result.is_err() {
        let mut command = driver.compose_command();
        command.args(["logs", "--tail", "35", "localton", "node-1", "node-2"]);
        if let Ok(output) = command.output().await {
            eprintln!("{}", output_text(&output));
        }
    }
    driver.delete().await?;
    result
}

async fn assert_membership(
    driver: &DockerNetwork,
    targets: &[Target],
    expected: &[bool],
) -> anyhow::Result<()> {
    use anyhow::ensure;
    let mut member_ids = None;
    for (target, &member) in targets.iter().zip(expected) {
        let state = driver.overlay_state(target).await?;
        ensure!(
            state.overlays.overlays.len() == usize::from(member),
            "Incorrect membership on {}",
            target.id
        );
        if member {
            let overlay = &state.overlays.overlays[0];
            ensure!(overlay.nodes.len() == 2);
            ensure!(
                overlay
                    .nodes
                    .iter()
                    .any(|node| node.adnl_id == state.engine.fullnode)
            );
            let ids = overlay
                .nodes
                .iter()
                .map(|node| node.adnl_id.clone())
                .collect::<Vec<_>>();
            if let Some(ref previous) = member_ids {
                ensure!(
                    *previous == ids,
                    "Participants received different overlay membership"
                );
            }
            member_ids = Some(ids);
        }
    }
    Ok(())
}

async fn wait_private_broadcasts(
    driver: &DockerNetwork,
    target: &Target,
    sender: &str,
) -> anyhow::Result<()> {
    use anyhow::{Context as _, bail};
    use std::time::{Duration, Instant};

    // TON publishes broadcast counters for the preceding minute. Observe both
    // an actual broadcast from genesis and incoming private-overlay throughput,
    // rather than treating persisted membership as proof of peer connectivity.
    let deadline = Instant::now() + Duration::from_secs(130);
    loop {
        let output = driver
            .overlay_console(target, "get-overlays-stats-json")
            .await?;
        let start = output
            .find("[\n")
            .context("Console did not return overlay stats JSON")?;
        let stats: serde_json::Value = serde_json::Deserializer::from_str(&output[start..])
            .into_iter()
            .next()
            .context("Empty overlay stats response")??;
        let overlay = stats.as_array().and_then(|overlays| {
            overlays.iter().find(|overlay| {
                overlay["scope"]["type"] == "custom-overlay"
                    && overlay["scope"]["name"] == "private"
            })
        });
        if let Some(overlay) = overlay {
            let received = overlay["broadcasts"].as_array().is_some_and(|broadcasts| {
                broadcasts.iter().any(|broadcast| {
                    broadcast["source"] == sender && positive_integer(&broadcast["count"])
                })
            });
            let traffic = positive_integer(&overlay["total_throughput"]["in_bytes_sec"]);
            if received && traffic {
                eprintln!(
                    "Private overlay on {} received broadcasts from genesis",
                    target.id
                );
                return Ok(());
            }
        }
        if Instant::now() >= deadline {
            bail!(
                "Private overlay did not receive genesis broadcasts on {}: {:?}",
                target.id,
                overlay
            );
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
}

fn positive_integer(value: &serde_json::Value) -> bool {
    value
        .as_u64()
        .or_else(|| value.as_str().and_then(|value| value.parse().ok()))
        .is_some_and(|value| value > 0)
}

async fn overlay_mtime(driver: &DockerNetwork, target: &Target) -> Result<String, Error> {
    let mut command = driver.compose_command();
    command.args([
        "exec",
        "-T",
        &target.service,
        "stat",
        "-c",
        "%y",
        "/var/lib/localton/node/db/custom-overlays.json",
    ]);
    let output = driver
        .command_output(
            command,
            "inspect overlay persistence",
            FAILURE_CODE,
            COMPOSE_NODE_COMMAND_TIMEOUT,
        )
        .await?;
    Ok(output_text(&output))
}

async fn move_server_key(driver: &DockerNetwork, service: &str, hide: bool) -> Result<(), Error> {
    let backup = "/var/lib/localton/node/certs/server.pub.overlay-test";
    let (from, to) = if hide {
        (SERVER_KEY, backup)
    } else {
        (backup, SERVER_KEY)
    };
    let mut command = driver.compose_command();
    command.args(["exec", "-T", service, "mv", from, to]);
    driver
        .run_command(
            command,
            "inject overlay test console failure",
            FAILURE_CODE,
            COMPOSE_NODE_COMMAND_TIMEOUT,
        )
        .await
}
