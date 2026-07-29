#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::time::Duration;

use acton_studio::{
    ContractRegistryStore, EnvironmentRuntime, LocalProcessEnvironmentRuntime,
    TESTNET_ENVIRONMENT_ID,
};
use expect_test::expect;
use serde_json::Value;
use tokio::time::{sleep, timeout};

#[tokio::test]
async fn project_artifacts_are_published_to_testnet_without_a_managed_environment() {
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
        vec![TESTNET_ENVIRONMENT_ID.to_owned()],
    )
    .await
    .expect("environment runtime");

    let registry_path = project_root
        .join(".studio/environments")
        .join(TESTNET_ENVIRONMENT_ID)
        .join("registry.json");
    let registry = wait_for_published_registry(&registry_path).await;
    sleep(Duration::from_millis(1_500)).await;

    let actual = format!(
        "managed environments: {}\nverified sources: {}\ncompiler ABIs: {}\nbuilds: {}",
        runtime.list().await.expect("environment list").len(),
        registry["verifiedSources"]
            .as_object()
            .map_or(0, serde_json::Map::len),
        registry["compilerAbis"]
            .as_object()
            .map_or(0, serde_json::Map::len),
        fs::read_to_string(&build_count_path)
            .expect("build count")
            .lines()
            .count(),
    );
    expect![[r"
        managed environments: 0
        verified sources: 1
        compiler ABIs: 1
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
