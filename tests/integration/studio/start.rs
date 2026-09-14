use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;
use acton_studio::load_studio_daemon_descriptor;

use super::{StudioCliProcess, reserve_studio_port};

#[test]
fn studio_start_port_conflict_is_reported_with_hint() {
    let project = ProjectBuilder::new("studio-port-conflict").build();
    let (_listener, port) = reserve_studio_port();
    let port = port.to_string();

    project
        .acton()
        .current_dir(project.path())
        .arg("studio")
        .arg("--port")
        .arg(&port)
        .arg("--no-open")
        .run()
        .failure()
        .assert_not_contains("Starting Acton Studio")
        .assert_stderr_contains("Failed to start Acton Studio on 127.0.0.1:")
        .assert_stderr_contains("Set another port with --port")
        .assert_stderr_contains("Or stop the process currently listening on that port")
        .assert_stderr_snapshot_matches(
            "integration/snapshots/studio/studio_start_port_conflict.stderr.txt",
        );
}

#[test]
fn studio_start_rejects_non_loopback_host() {
    let project = ProjectBuilder::new("studio-public-host").build();

    project
        .acton()
        .current_dir(project.path())
        .args(["studio", "--host", "0.0.0.0", "--no-open"])
        .run()
        .failure()
        .assert_not_contains("Starting Acton Studio")
        .assert_stderr_snapshot_matches(
            "integration/snapshots/studio/studio_start_rejects_non_loopback_host.stderr.txt",
        );
}

#[cfg(unix)]
#[test]
fn studio_start_serves_workspace_and_registers_for_reporting() {
    let project = ProjectBuilder::new("studio-cli-start").build();
    let mut studio = StudioCliProcess::start(&project);
    let studio_pid = studio.id();

    let info = studio.wait_for_info();
    let workspace = info
        .workspace
        .expect("Studio started in a project must publish workspace info");
    let descriptor = load_studio_daemon_descriptor(project.path())
        .expect("Studio daemon descriptor must be readable")
        .expect("Studio CLI must register itself for test reporting");

    assert_eq!(info.protocol_version, 1);
    assert!(!info.server_version.is_empty());
    assert_eq!(workspace.name, "studio-cli-start");
    assert!(workspace.wallet_names.is_empty());
    assert_eq!(descriptor.url, studio.url());
    assert_eq!(descriptor.pid, studio_pid);

    studio.stop();
    assert!(
        load_studio_daemon_descriptor(project.path())
            .expect("Studio daemon descriptor must remain readable after shutdown")
            .is_none()
    );
}

#[cfg(unix)]
#[test]
fn studio_startup_accounts_use_the_submitted_selection_instead_of_cli_defaults() {
    use crate::support::toncenter::DEPLOYER_WALLET_CONFIG;
    use expect_test::expect;
    use serde_json::{Value, json};
    use std::time::{Duration, Instant};

    let project = ProjectBuilder::new("studio-startup-selection").build();
    std::fs::write(project.path().join("wallets.toml"), DEPLOYER_WALLET_CONFIG)
        .expect("public fixture wallet");
    let manifest_path = project.path().join("Acton.toml");
    let mut manifest = std::fs::read_to_string(&manifest_path).expect("manifest");
    manifest.push_str("\n[localnet]\naccounts = [\"deployer\"]\n");
    std::fs::write(manifest_path, manifest).expect("CLI startup defaults");

    let names = |payload: &Value| {
        payload["result"]
            .as_array()
            .expect("startup accounts array")
            .iter()
            .map(|wallet| wallet["name"].clone())
            .collect::<Vec<_>>()
    };

    // Direct CLI starts still inherit project defaults when --accounts is omitted.
    let cli_node = project.localnet().arg("--no-mining").start();
    let cli_accounts = names(&cli_node.get_json("/acton_getStartupAccounts"));
    cli_node.stop();

    let mut studio = StudioCliProcess::start(&project);
    let form_defaults = studio
        .wait_for_info()
        .workspace
        .expect("project workspace")
        .default_startup_accounts;
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .expect("Studio client");
    let mut environments = Vec::new();

    for selected in [Vec::<String>::new(), vec!["deployer".to_owned()]] {
        let created: Value = client
            .post(format!("{}/api/v1/environments", studio.url()))
            .json(&json!({
                "name": "Selected wallets",
                "config": {
                    "kind": "actonSimulatedLocalnet",
                    "accounts": selected,
                    "noMining": true,
                },
            }))
            .send()
            .expect("create request")
            .error_for_status()
            .expect("create accepted")
            .json()
            .expect("created environment");
        let id = created["id"].as_str().expect("environment ID");
        let url = format!("{}/api/v1/environments/{id}", studio.url());
        let startup_accounts = || {
            let deadline = Instant::now() + Duration::from_secs(20);
            loop {
                let environment: Value = client
                    .get(&url)
                    .send()
                    .expect("environment request")
                    .error_for_status()
                    .expect("environment response")
                    .json()
                    .expect("environment JSON");
                match environment["status"].as_str() {
                    Some("running") => break,
                    Some("failed") => panic!("startup failed: {environment}"),
                    _ if Instant::now() >= deadline => panic!("startup timed out: {environment}"),
                    _ => std::thread::sleep(Duration::from_millis(50)),
                }
            }
            let payload: Value = client
                .get(format!("{url}/rpc/acton_getStartupAccounts"))
                .send()
                .expect("startup accounts request")
                .error_for_status()
                .expect("startup accounts response")
                .json()
                .expect("startup accounts JSON");
            names(&payload)
        };
        let initial = startup_accounts();

        // Saved Studio selections remain authoritative on subsequent process launches.
        for action in ["stop", "restart"] {
            client
                .post(format!("{url}/{action}"))
                .send()
                .expect("lifecycle request")
                .error_for_status()
                .expect("lifecycle accepted");
        }
        environments.push(json!({
            "selected": selected,
            "initial": initial,
            "restarted": startup_accounts(),
        }));
        client
            .delete(&url)
            .send()
            .expect("delete request")
            .error_for_status()
            .expect("environment removed");
    }
    studio.stop();

    expect![[r#"
        {
          "cliAccounts": [
            "deployer"
          ],
          "environments": [
            {
              "initial": [],
              "restarted": [],
              "selected": []
            },
            {
              "initial": [
                "deployer"
              ],
              "restarted": [
                "deployer"
              ],
              "selected": [
                "deployer"
              ]
            }
          ],
          "formDefaults": [
            "deployer"
          ]
        }"#]]
    .assert_eq(
        &serde_json::to_string_pretty(&json!({
            "cliAccounts": cli_accounts,
            "formDefaults": form_defaults,
            "environments": environments,
        }))
        .expect("startup selection summary"),
    );
}

#[cfg(unix)]
#[test]
fn studio_start_rejects_a_second_instance_for_the_same_project() {
    let project = ProjectBuilder::new("studio-duplicate-instance").build();
    let mut first = StudioCliProcess::start(&project);

    let (second_listener, second_port) = reserve_studio_port();
    drop(second_listener);
    let second_port_arg = second_port.to_string();
    project
        .acton()
        .current_dir(project.path())
        .args(["studio", "--port", &second_port_arg, "--no-open"])
        .run()
        .failure()
        .assert_not_contains("Starting Acton Studio")
        .assert_stderr_contains("Another Acton Studio instance is already running")
        .assert_stderr_snapshot_matches(
            "integration/snapshots/studio/studio_start_rejects_duplicate_instance.stderr.txt",
        );

    let info = first.wait_for_info();
    assert_eq!(
        info.workspace
            .expect("first Studio instance must remain available")
            .name,
        "studio-duplicate-instance"
    );
    first.stop();
}

#[cfg(unix)]
#[test]
fn studio_start_works_without_an_acton_manifest() {
    let project = ProjectBuilder::new("studio-without-manifest")
        .without_acton_toml()
        .build();
    let mut studio = StudioCliProcess::start(&project);

    let info = studio.wait_for_info();
    assert!(info.workspace.is_none());
    let descriptor = load_studio_daemon_descriptor(project.path())
        .expect("standalone Studio descriptor must be readable")
        .expect("standalone Studio must publish its descriptor");
    assert_eq!(descriptor.url, studio.url());
    assert_eq!(descriptor.pid, studio.id());

    studio.stop();
    assert!(
        load_studio_daemon_descriptor(project.path())
            .expect("standalone Studio descriptor must remain readable after shutdown")
            .is_none()
    );
}
