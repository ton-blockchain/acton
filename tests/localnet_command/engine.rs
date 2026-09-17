//! Engine request snapshots cover deployment semantics without a Docker CLI.

use super::*;
use std::collections::BTreeMap;

fn events(service: &Service) -> Vec<Value> {
    std::fs::read_to_string(service.root.path().join("engine-events"))
        .expect("Engine requests")
        .lines()
        .map(|line| serde_json::from_str(line).expect("Engine request"))
        .collect()
}

#[tokio::test]
async fn engine_reuses_containers_and_preserves_topology_without_cli() {
    let mut service = Service::start(false).await;
    let client = service.client().await;
    let ports = service.network.network.config.ports();
    let v2 = api_listener(ports.api_v2).await;
    let v3 = api_listener(ports.api_v3).await;
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", ports.observability))
        .await
        .expect("observability port");
    let app = axum::Router::new().fallback(|| async {
        axum::Json(json!({"nodes":[{"name":"peer", "online":true, "running":true, "head_seqno":10,
            "head_observed_at":std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs()}]}))
    });
    let observability =
        tokio::spawn(async move { axum::serve(listener, app).await.expect("observability API") });
    cli(&service.state(), &["start", "integration"]).await;
    cli(&service.state(), &["node", "integration", "add", "peer"]).await;
    let created_before_restart = events(&service)
        .iter()
        .filter(|e| e["path"] == "/containers/create")
        .count();
    let stopping: Operation = client
        .request(Method::POST, "/v1/network/stop", None)
        .await
        .expect("stop accepted");
    let stopped = tokio::time::timeout(Duration::from_secs(20), async {
        loop {
            let operation = client
                .operation(&stopping.id)
                .await
                .expect("stop operation");
            if operation.status != acton_localnet::OperationStatus::Running {
                break operation;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("network stopped");
    expect![["Completed"]].assert_eq(&format!("{:?}", stopped.status));
    cli(&service.state(), &["start", "integration"]).await;
    let requests = events(&service);
    let definitions: BTreeMap<_, _> = requests
        .iter()
        .filter(|e| e["path"] == "/containers/create")
        .map(|e| {
            (
                e["body"]["Labels"]["org.ton.acton.localnet.service"]
                    .as_str()
                    .expect("service"),
                &e["body"],
            )
        })
        .collect();
    let project = definitions["localton"]["Labels"]["org.ton.acton.localnet.project"]
        .as_str()
        .expect("project");
    let localton = &definitions["localton"];
    let worker = &definitions["v3-worker"];
    let postgres = &definitions["postgres"];
    let node = &definitions["node-1"];
    let bindings = localton["HostConfig"]["PortBindings"]
        .as_object()
        .expect("ports");
    let binds = |config: &Value| {
        config["HostConfig"]["Binds"]
            .as_array()
            .expect("mounts")
            .iter()
            .map(|v| v.as_str().expect("bind").replace(project, "<project>"))
            .collect::<Vec<_>>()
    };
    let summary = json!({
        "rawPayloadsLogged": std::fs::read_to_string(service.root.path().join("logs/debug.log")).unwrap_or_default().contains("Decoded into string:"),
        "cliAbsent": !service.root.path().join("bin/docker").exists(),
        "containersBeforeRestart": created_before_restart,
        "containersAfterRestart": requests.iter().filter(|e| e["path"] == "/containers/create").count(),
        "services": definitions.keys().collect::<Vec<_>>(),
        "owner": {
            "command": localton["Cmd"],
            "binds": binds(localton),
            "ports": bindings.keys().collect::<Vec<_>>(),
            "loopbackOnly": bindings.values().all(|v| v[0]["HostIp"] == "127.0.0.1"),
            "health": localton["Healthcheck"],
            "stopTimeout": localton["StopTimeout"],
            "restart": localton["HostConfig"]["RestartPolicy"],
            "security": localton["HostConfig"]["SecurityOpt"],
        },
        "postgres": {"image": postgres["Image"], "sharedMemory": postgres["HostConfig"]["ShmSize"], "environment": postgres["Env"]},
        "worker": {"user": worker["User"], "limits": worker["HostConfig"]["Ulimits"], "binds": binds(worker)},
        "node": {"sharesOwnerNetwork": node["HostConfig"]["NetworkMode"].as_str().is_some_and(|s| s.starts_with("container:")), "binds": binds(node)},
        "networkAliases": definitions.values().filter_map(|c| c["NetworkingConfig"]["EndpointsConfig"].as_object())
            .flat_map(|endpoints| endpoints.values().map(|e| e["Aliases"].clone())).collect::<Vec<_>>(),
    });
    expect![[r#"
        {
          "cliAbsent": true,
          "containersAfterRestart": 10,
          "containersBeforeRestart": 10,
          "networkAliases": [
            [
              "localton"
            ],
            [
              "postgres"
            ],
            [
              "redis"
            ],
            [
              "v3-account-scanner"
            ],
            [
              "v3-api"
            ],
            [
              "v3-basechain-bootstrap"
            ],
            [
              "v3-classifier"
            ],
            [
              "v3-migrations"
            ],
            [
              "v3-worker"
            ]
          ],
          "node": {
            "binds": [
              "<project>_node-1-state:/var/lib/localton"
            ],
            "sharesOwnerNetwork": true
          },
          "owner": {
            "binds": [
              "<project>_localton-state:/var/lib/localton",
              "<project>_localton-snapshots:/var/lib/localton-snapshots"
            ],
            "command": [
              "bootstrap",
              "--state-dir",
              "/var/lib/localton",
              "--block-time",
              "400",
              "--election-time",
              "30",
              "--ton-http-api",
              "--observability-bind",
              "0.0.0.0"
            ],
            "health": {
              "Interval": 10000000000,
              "Retries": 12,
              "StartInterval": 1000000000,
              "StartPeriod": 240000000000,
              "Test": [
                "CMD",
                "curl",
                "--fail",
                "--silent",
                "--show-error",
                "http://127.0.0.1:18002/api/v2/getMasterchainInfo"
              ],
              "Timeout": 3000000000
            },
            "loopbackOnly": true,
            "ports": [
              "18000/tcp",
              "18001/tcp",
              "18002/tcp",
              "18007/tcp"
            ],
            "restart": {
              "Name": "unless-stopped"
            },
            "security": [
              "no-new-privileges:true"
            ],
            "stopTimeout": 60
          },
          "postgres": {
            "environment": [
              "POSTGRES_DB=ton_index",
              "POSTGRES_HOST_AUTH_METHOD=trust",
              "POSTGRES_USER=postgres"
            ],
            "image": "postgres:17-alpine",
            "sharedMemory": 536870912
          },
          "rawPayloadsLogged": false,
          "services": [
            "localton",
            "node-1",
            "postgres",
            "redis",
            "v3-account-scanner",
            "v3-api",
            "v3-basechain-bootstrap",
            "v3-classifier",
            "v3-migrations",
            "v3-worker"
          ],
          "worker": {
            "binds": [
              "<project>_localton-state:/var/lib/localton",
              "<project>_ton-index-workdir:/var/lib/ton-indexer/work"
            ],
            "limits": [
              {
                "Hard": 1000000,
                "Name": "nofile",
                "Soft": 1000000
              }
            ],
            "user": "0:0"
          }
        }"#]]
    .assert_eq(&serde_json::to_string_pretty(&summary).expect("Engine topology snapshot"));
    service.stop(&client).await;
    v2.abort();
    v3.abort();
    observability.abort();
    let _ = tokio::join!(v2, v3, observability);
    drop(service);
}

#[tokio::test]
async fn docker_context_is_resolved_locally_and_pinned_across_restart() {
    use sha2::{Digest, Sha256};

    let mut service = Service::start(false).await;
    let client = service.client().await;
    service.stop(&client).await;
    let directory = service.root.path().join("docker-config");
    let metadata = directory
        .join("contexts/meta")
        .join(format!("{:x}", Sha256::digest(b"fixture-context")));
    std::fs::create_dir_all(&metadata).expect("context directory");
    let host =
        std::fs::read_to_string(service.root.path().join("docker-host")).expect("engine host");
    std::fs::write(
        metadata.join("meta.json"),
        serde_json::to_vec(&json!({"Endpoints":{"docker":{"Host":host,"SkipTLSVerify":false}}}))
            .unwrap(),
    )
    .expect("context metadata");
    std::fs::write(
        directory.join("config.json"),
        r#"{"currentContext":"fixture-context"}"#,
    )
    .expect("default context");
    let v2 = api_listener(service.network.network.config.ports().api_v2).await;
    let v3 = api_listener(service.network.network.config.ports().api_v3).await;
    let mut targets = Vec::new();
    for explicit in [false, true] {
        let log =
            std::fs::File::create(service.root.path().join("service.log")).expect("service log");
        let mut command = acton(service.root.path(), &["serve"]);
        command.env_remove("DOCKER_HOST");
        if explicit {
            // Once persisted, a different process environment cannot redirect
            // an existing deployment to another engine or context.
            command
                .env("DOCKER_HOST", "tcp://127.0.0.1:1")
                .env("DOCKER_CONTEXT", "missing-context");
        }
        service.child = Command::from(command)
            .stdout(Stdio::from(log.try_clone().unwrap()))
            .stderr(Stdio::from(log))
            .kill_on_drop(true)
            .spawn()
            .expect("control service with context");
        let client = service.client().await;
        cli(&service.state(), &["start", "integration"]).await;
        let descriptor: Value = serde_json::from_slice(
            &std::fs::read(service.network.path.join("runtime.json")).unwrap(),
        )
        .unwrap();
        targets.push(descriptor["dockerTarget"].clone());
        service.stop(&client).await;
    }
    expect![[r#"
        [
          {
            "kind": "context",
            "value": "fixture-context"
          },
          {
            "kind": "context",
            "value": "fixture-context"
          }
        ]"#]]
    .assert_eq(&serde_json::to_string_pretty(&targets).unwrap());
    v2.abort();
    v3.abort();
    let _ = tokio::join!(v2, v3);
    drop(service);
}
