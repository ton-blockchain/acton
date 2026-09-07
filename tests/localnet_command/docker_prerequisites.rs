//! The real service classifies Docker failures and preserves diagnostics before deployment.

use super::*;

#[tokio::test]
async fn docker_failures_keep_their_cause_and_do_not_materialize_a_deployment() {
    let mut service = Service::start(false).await;
    let client = service.client().await;
    let marker = service.root.path().join("docker-error");
    let mut outcomes = Vec::new();

    for (scenario, diagnostic) in [
        (
            "stopped",
            "Cannot connect to the Docker daemon at unix:///var/run/docker.sock. Is the docker daemon running?",
        ),
        (
            "windows-stopped",
            "error during connect: open //./pipe/dockerDesktopLinuxEngine: The system cannot find the file specified",
        ),
        (
            "permissions",
            "permission denied while trying to connect to the Docker daemon socket at unix:///var/run/docker.sock",
        ),
        (
            "tls",
            "error during connect: x509: certificate signed by unknown authority",
        ),
        ("context", "context production: context not found"),
        (
            "remote",
            "Cannot connect to the Docker daemon at tcp://docker.example:2376. Is the docker daemon running?",
        ),
        (
            "dns",
            "error during connect: dial tcp: lookup docker.example: no such host",
        ),
        (
            "disk",
            "Error response from daemon: no space left on device",
        ),
        (
            "port",
            "Error response from daemon: Bind for 0.0.0.0:18000 failed: port is already allocated",
        ),
        (
            "registry-auth",
            "Error response from daemon: pull access denied for localton, repository does not exist or may require docker login",
        ),
        (
            "image",
            "no matching manifest for linux/arm64/v8 in the manifest list entries",
        ),
        (
            "registry-limit",
            "toomanyrequests: You have reached your unauthenticated pull rate limit",
        ),
        (
            "unknown",
            "Unexpected Docker API response: fixture-specific failure 731",
        ),
    ] {
        std::fs::write(&marker, diagnostic).expect("inject only this fixture's Docker failure");
        let accepted: Operation = client
            .request(Method::POST, "/v1/network/start", None)
            .await
            .expect("start accepted");
        let operation = tokio::time::timeout(Duration::from_secs(20), async {
            loop {
                let operation: Operation = client
                    .request(
                        Method::GET,
                        &format!("/v1/operations/{}", accepted.id),
                        None,
                    )
                    .await
                    .expect("operation remains readable");
                if operation.status != acton_localnet::OperationStatus::Running {
                    break operation;
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .expect("availability check completes");
        let network = client.network().await.expect("network remains accessible");
        let error = operation.error.expect("reported error");
        let log = std::fs::read_to_string(&operation.log_path).expect("published log exists");

        outcomes.push(json!({
            "scenario": scenario,
            "code": operation.error_code,
            "status": network.status,
            "summary": error.split("\n\n").next(),
            "diagnosticsRetained": error.contains(diagnostic) && log.contains(diagnostic),
            "deploymentCreated": service.network.path.join("runtime.json").exists()
                || service.network.path.join("compose.yaml").exists(),
        }));
    }

    std::fs::remove_file(marker).expect("restore fixture Docker");
    service.stop(&client).await;
    drop(service);
    expect![[r#"
        [
          {
            "code": "docker_engine_unavailable",
            "deploymentCreated": false,
            "diagnosticsRetained": true,
            "scenario": "stopped",
            "status": "failed",
            "summary": "Docker is not running\nStart Docker Desktop or your Docker Engine service, wait until it is ready, then retry"
          },
          {
            "code": "docker_engine_unavailable",
            "deploymentCreated": false,
            "diagnosticsRetained": true,
            "scenario": "windows-stopped",
            "status": "failed",
            "summary": "Docker is not running\nStart Docker Desktop or your Docker Engine service, wait until it is ready, then retry"
          },
          {
            "code": "docker_permission_denied",
            "deploymentCreated": false,
            "diagnosticsRetained": true,
            "scenario": "permissions",
            "status": "failed",
            "summary": "Access to Docker was denied\nGrant your user access to the selected Docker engine, then retry"
          },
          {
            "code": "docker_tls_failed",
            "deploymentCreated": false,
            "diagnosticsRetained": true,
            "scenario": "tls",
            "status": "failed",
            "summary": "Docker TLS verification failed\nCheck the certificates and TLS settings for the selected Docker context"
          },
          {
            "code": "docker_context_unavailable",
            "deploymentCreated": false,
            "diagnosticsRetained": true,
            "scenario": "context",
            "status": "failed",
            "summary": "The selected Docker context is unavailable\nCheck `docker context ls` and restore or repair the selected context"
          },
          {
            "code": "docker_connection_failed",
            "deploymentCreated": false,
            "diagnosticsRetained": true,
            "scenario": "remote",
            "status": "failed",
            "summary": "The selected Docker endpoint could not be reached\nCheck the Docker context or DOCKER_HOST and verify that the target engine is reachable"
          },
          {
            "code": "docker_connection_failed",
            "deploymentCreated": false,
            "diagnosticsRetained": true,
            "scenario": "dns",
            "status": "failed",
            "summary": "Docker could not reach the requested endpoint\nCheck the host, network, VPN and proxy settings shown in the diagnostic details, then retry"
          },
          {
            "code": "docker_storage_full",
            "deploymentCreated": false,
            "diagnosticsRetained": true,
            "scenario": "disk",
            "status": "failed",
            "summary": "Docker has run out of disk space\nFree space in Docker's storage or increase its disk limit, then retry"
          },
          {
            "code": "docker_port_in_use",
            "deploymentCreated": false,
            "diagnosticsRetained": true,
            "scenario": "port",
            "status": "failed",
            "summary": "A network port is already in use\nStop the conflicting service or choose unused ports for this localnet"
          },
          {
            "code": "docker_registry_access_denied",
            "deploymentCreated": false,
            "diagnosticsRetained": true,
            "scenario": "registry-auth",
            "status": "failed",
            "summary": "Docker could not access the image registry\nCheck the image name and sign in to its registry with `docker login` if it requires authentication"
          },
          {
            "code": "docker_image_unavailable",
            "deploymentCreated": false,
            "diagnosticsRetained": true,
            "scenario": "image",
            "status": "failed",
            "summary": "The Docker image is unavailable for this platform\nCheck the image tag and that it supports your machine's architecture"
          },
          {
            "code": "docker_registry_rate_limited",
            "deploymentCreated": false,
            "diagnosticsRetained": true,
            "scenario": "registry-limit",
            "status": "failed",
            "summary": "The image registry rate limit was reached\nSign in to the registry or wait before retrying the image download"
          },
          {
            "code": "docker_check_failed",
            "deploymentCreated": false,
            "diagnosticsRetained": true,
            "scenario": "unknown",
            "status": "failed",
            "summary": "Docker could not complete its availability check\nRun `docker info --format {{.ServerVersion}}` with the selected Docker context to inspect the failure"
          }
        ]"#]].assert_eq(&serde_json::to_string_pretty(&outcomes).expect("failure summaries"));
}

#[tokio::test]
async fn missing_tools_and_timeout_remain_actionable_after_cli_shutdown() {
    let mut service = Service::start(false).await;
    let client = service.client().await;
    service.stop(&client).await;
    let empty_path = service.root.path().join("empty-path");
    std::fs::create_dir(&empty_path).expect("isolated PATH");
    let mut results = Vec::new();

    for scenario in [
        "missing",
        "missing-context",
        "compose-unavailable",
        "docker-timeout",
    ] {
        let marker = service.root.path().join(scenario);
        let mut command = acton(service.root.path(), &["start", "integration"]);
        if scenario.starts_with("missing") {
            command.env("PATH", &empty_path);
            if scenario == "missing-context" {
                command
                    .env_remove("DOCKER_CONTEXT")
                    .env_remove("DOCKER_HOST");
            }
        } else {
            std::fs::write(&marker, "").expect("Docker failure mode");
        }

        let output = tokio::time::timeout(Duration::from_secs(25), Command::from(command).output())
            .await
            .expect("bounded startup failure")
            .expect("CLI output");
        let network: Network = serde_json::from_slice(
            &std::fs::read(service.network.path.join("network.json")).expect("persisted failure"),
        )
        .expect("network");
        let operation = network.operation.expect("failed operation");
        let error = operation.error.expect("actionable error");
        let summary = error.lines().take(2).collect::<Vec<_>>().join("\n");
        let log = std::fs::read_to_string(&operation.log_path)
            .expect("diagnostic log exists even before Docker starts");
        results.push(json!({
            "scenario": scenario,
            "code": operation.error_code,
            "status": network.status,
            "error": summary,
            "cliFailed": !output.status.success(),
            "cliExplainsRecovery": String::from_utf8_lossy(&output.stderr).contains(&summary),
            "logExplainsRecovery": log.contains(&summary),
            "descriptorCreated": service.network.path.join("runtime.json").exists(),
            "composeCreated": service.network.path.join("compose.yaml").exists(),
            "containersStarted": service.root.path().join("events").exists(),
            "serviceLeftRunning": service.network.path.join("service.json").exists(),
        }));
        if !scenario.starts_with("missing") {
            std::fs::remove_file(marker).expect("clear only this test's Docker failure marker");
        }
    }

    drop(service);
    expect![[r#"
        [
          {
            "cliExplainsRecovery": true,
            "cliFailed": true,
            "code": "docker_not_found",
            "composeCreated": false,
            "containersStarted": false,
            "descriptorCreated": false,
            "error": "Docker CLI was not found on PATH\nInstall Docker Desktop or Docker Engine with Compose v2 and make `docker` available to Acton",
            "logExplainsRecovery": true,
            "scenario": "missing",
            "serviceLeftRunning": false,
            "status": "failed"
          },
          {
            "cliExplainsRecovery": true,
            "cliFailed": true,
            "code": "docker_not_found",
            "composeCreated": false,
            "containersStarted": false,
            "descriptorCreated": false,
            "error": "Docker CLI was not found on PATH\nInstall Docker Desktop or Docker Engine with Compose v2 and make `docker` available to Acton",
            "logExplainsRecovery": true,
            "scenario": "missing-context",
            "serviceLeftRunning": false,
            "status": "failed"
          },
          {
            "cliExplainsRecovery": true,
            "cliFailed": true,
            "code": "docker_compose_unavailable",
            "composeCreated": false,
            "containersStarted": false,
            "descriptorCreated": false,
            "error": "Docker Compose is not available\nInstall or enable Compose v2, then retry",
            "logExplainsRecovery": true,
            "scenario": "compose-unavailable",
            "serviceLeftRunning": false,
            "status": "failed"
          },
          {
            "cliExplainsRecovery": true,
            "cliFailed": true,
            "code": "docker_check_failed",
            "composeCreated": false,
            "containersStarted": false,
            "descriptorCreated": false,
            "error": "Docker did not respond within 10 seconds\nCheck the selected Docker context and connection, then retry",
            "logExplainsRecovery": true,
            "scenario": "docker-timeout",
            "serviceLeftRunning": false,
            "status": "failed"
          }
        ]"#]]
        .assert_eq(&serde_json::to_string_pretty(&results).expect("prerequisite outcomes"));
}
