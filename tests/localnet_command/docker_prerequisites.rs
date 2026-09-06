//! Exercise real CLI startup with unavailable Docker, without touching the host daemon.

use super::*;

#[tokio::test]
async fn docker_prerequisites_fail_before_materializing_or_pulling_and_keep_recovery_details() {
    let mut service = Service::start(false).await;
    let client = service.client().await;
    service.stop(&client).await;
    let empty_path = service.root.path().join("empty-path");
    std::fs::create_dir(&empty_path).expect("isolated PATH");
    let mut results = Vec::new();

    for scenario in [
        "missing",
        "missing-context",
        "docker-unavailable",
        "docker-denied",
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

    expect![[r#"
        [
          {
            "cliExplainsRecovery": true,
            "cliFailed": true,
            "code": "docker_not_found",
            "composeCreated": false,
            "containersStarted": false,
            "descriptorCreated": false,
            "error": "Docker CLI was not found on PATH\nNext step: Install Docker Desktop on macOS/Windows, or Docker Engine with the Compose plugin on Linux, then restart Acton from a terminal where `docker info` works",
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
            "error": "Docker CLI was not found on PATH\nNext step: Install Docker Desktop on macOS/Windows, or Docker Engine with the Compose plugin on Linux, then restart Acton from a terminal where `docker info` works",
            "logExplainsRecovery": true,
            "scenario": "missing-context",
            "serviceLeftRunning": false,
            "status": "failed"
          },
          {
            "cliExplainsRecovery": true,
            "cliFailed": true,
            "code": "docker_engine_unavailable",
            "composeCreated": false,
            "containersStarted": false,
            "descriptorCreated": false,
            "error": "Docker Engine is not reachable\nNext step: Start Docker Desktop or your Docker Engine service and wait until `docker info` succeeds, then retry this operation",
            "logExplainsRecovery": true,
            "scenario": "docker-unavailable",
            "serviceLeftRunning": false,
            "status": "failed"
          },
          {
            "cliExplainsRecovery": true,
            "cliFailed": true,
            "code": "docker_permission_denied",
            "composeCreated": false,
            "containersStarted": false,
            "descriptorCreated": false,
            "error": "Access to Docker was denied\nNext step: Grant your user access to Docker, verify `docker info` from the terminal that runs Acton, then retry this operation",
            "logExplainsRecovery": true,
            "scenario": "docker-denied",
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
            "error": "Docker Compose is not available\nNext step: Install or enable Docker Compose v2, verify `docker compose version`, then retry this operation",
            "logExplainsRecovery": true,
            "scenario": "compose-unavailable",
            "serviceLeftRunning": false,
            "status": "failed"
          },
          {
            "cliExplainsRecovery": true,
            "cliFailed": true,
            "code": "docker_engine_unavailable",
            "composeCreated": false,
            "containersStarted": false,
            "descriptorCreated": false,
            "error": "Docker Engine is not reachable\nNext step: Start Docker Desktop or your Docker Engine service and wait until `docker info` succeeds, then retry this operation",
            "logExplainsRecovery": true,
            "scenario": "docker-timeout",
            "serviceLeftRunning": false,
            "status": "failed"
          }
        ]"#]]
        .assert_eq(&serde_json::to_string_pretty(&results).expect("prerequisite outcomes"));
}
