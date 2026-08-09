#![cfg(unix)]

use std::fmt::Write as _;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::time::Duration;

use acton_studio::{
    ContractRegistryStore, EnvironmentRuntime, LocalProcessEnvironmentRuntime,
    PUBLIC_TON_ENVIRONMENT_IDS,
};
use expect_test::expect;
use serde_json::Value;
use tokio::time::{sleep, timeout};

#[tokio::test]
async fn project_artifacts_are_published_to_public_networks_without_a_managed_environment() {
    let temp = tempfile::tempdir_in("/tmp").expect("temporary workspace");
    let project_root = temp.path().join("project");
    fs::create_dir(&project_root).expect("project directory");
    fs::write(project_root.join("Acton.toml"), "[contracts]\n").expect("Acton manifest");
    fs::write(project_root.join("counter.tolk"), "fun main() {}\n").expect("contract source");

    let build_count_path = temp.path().join("build-count.txt");
    let executable = temp.path().join("fake-acton");
    write_artifact_build_executable(&executable, &build_count_path);

    let runtime = LocalProcessEnvironmentRuntime::open(
        &executable,
        &project_root,
        ContractRegistryStore::for_project(&project_root),
        PUBLIC_TON_ENVIRONMENT_IDS
            .iter()
            .map(ToString::to_string)
            .collect(),
    )
    .await
    .expect("environment runtime");

    let mut registries = Vec::new();
    for environment_id in PUBLIC_TON_ENVIRONMENT_IDS {
        let registry_path = project_root
            .join(".studio/environments")
            .join(environment_id)
            .join("registry.json");
        registries.push((
            environment_id,
            wait_for_published_registry(&registry_path).await,
        ));
    }
    sleep(Duration::from_millis(1_500)).await;

    let mut actual = format!(
        "managed environments: {}",
        runtime.list().await.expect("environment list").len(),
    );
    for (environment_id, registry) in registries {
        write!(
            actual,
            "\n{environment_id} verified sources: {}\n{environment_id} compiler ABIs: {}",
            registry["verifiedSources"]
                .as_object()
                .map_or(0, serde_json::Map::len),
            registry["compilerAbis"]
                .as_object()
                .map_or(0, serde_json::Map::len),
        )
        .expect("artifact snapshot must be writable");
    }
    write!(
        actual,
        "\nbuilds: {}",
        fs::read_to_string(&build_count_path)
            .expect("build count")
            .lines()
            .count(),
    )
    .expect("artifact snapshot must be writable");
    expect![[r"
        managed environments: 0
        testnet verified sources: 1
        testnet compiler ABIs: 1
        mainnet verified sources: 1
        mainnet compiler ABIs: 1
        builds: 1"]]
    .assert_eq(&actual);

    runtime.shutdown().await.expect("runtime shutdown");
}

async fn wait_for_published_registry(path: &Path) -> Value {
    timeout(Duration::from_secs(8), async {
        loop {
            if let Ok(bytes) = fs::read(path)
                && let Ok(registry) = serde_json::from_slice::<Value>(&bytes)
                && registry["verifiedSources"]
                    .as_object()
                    .is_some_and(|sources| !sources.is_empty())
                && registry["compilerAbis"]
                    .as_object()
                    .is_some_and(|abis| !abis.is_empty())
            {
                return registry;
            }
            sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("artifact registry was not published at {}", path.display()))
}

fn write_artifact_build_executable(path: &Path, build_count_path: &Path) {
    let script = format!(
        r#"#!/bin/sh
output_sources=
while [ "$#" -gt 0 ]; do
  if [ "$1" = "--output-sources" ]; then
    shift
    output_sources="$1"
  fi
  shift
done
mkdir -p "$output_sources"
cat > "$output_sources/counter.source.json" <<'JSON'
{{"code_hash":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","bundle":{{"source_bundle_hash":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb","compiler_abi":{{"abi_schema_version":"1.0","contract_name":"Counter","declarations":[]}}}}}}
JSON
printf 'build\n' >> '{}'
"#,
        shell_single_quote(build_count_path),
    );
    fs::write(path, script).expect("fake Acton executable");
    let mut permissions = fs::metadata(path)
        .expect("executable metadata")
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).expect("executable permissions");
}

fn shell_single_quote(path: &Path) -> String {
    path.to_string_lossy().replace('\'', "'\"'\"'")
}
