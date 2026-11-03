use super::{acton, catalog};
use expect_test::expect;

#[tokio::test]
async fn genesis_file_flags_override_environment_and_validate_effective_timings() {
    let root = tempfile::tempdir_in("/tmp").unwrap();
    let file = root.path().join("genesis.toml");
    let env_file = root.path().join("env.toml");
    std::fs::write(
        &file,
        "global_id = -42\nelected_for_seconds = 60\nsimplex_target_rate_ms = 0",
    )
    .unwrap();
    std::fs::write(&env_file, "global_id = -11").unwrap();
    let output = acton(
        root.path(),
        &[
            "create",
            "custom",
            "--genesis",
            file.to_str().unwrap(),
            "--block-time-ms",
            "500",
            "--election-time-seconds",
            "40",
            "--json",
        ],
    )
    .env("ACTON_LOCALNET_GENESIS", &env_file)
    .output()
    .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let output = acton(root.path(), &["create", "env", "--json"])
        .env("ACTON_LOCALNET_GENESIS", &env_file)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let invalid = acton(
        root.path(),
        &["create", "invalid", "--genesis", file.to_str().unwrap()],
    )
    .env_remove("ACTON_LOCALNET_GENESIS")
    .output()
    .unwrap();
    let networks = catalog::list(&root.path().join(".acton-localnet"))
        .await
        .unwrap();
    let values = networks
        .iter()
        .map(|saved| {
            let config = saved.network.config.genesis.as_ref().unwrap();
            format!(
                "{}: {} {} {} {}",
                saved.network.name,
                config.global_id,
                config.simplex_target_rate_ms,
                config.elected_for_seconds,
                config.election_start_before_seconds
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    expect![[r"
        custom: -42 500 40 30
        env: -11 1000 120 90
        invalid rejected: true
        service started: false"]]
    .assert_eq(&format!(
        "{values}\ninvalid rejected: {}\nservice started: {}",
        !invalid.status.success(),
        networks
            .iter()
            .any(|network| network.path.join("service.json").exists())
    ));
}

#[test]
fn genesis_schema_is_available_without_a_project() {
    let root = tempfile::tempdir_in("/tmp").unwrap();
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_acton"))
        .args(["meta", "get-schema", "genesis-toml"])
        .current_dir(root.path())
        .output()
        .unwrap();
    expect![["true true true"]].assert_eq(&format!(
        "{} {} {}",
        output.status.success(),
        output.stdout == acton_localnet::GENESIS_SCHEMA_JSON.as_bytes(),
        output.stderr.is_empty()
    ));
}
