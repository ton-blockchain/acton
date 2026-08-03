use std::io::Write;
use std::net::SocketAddr;
use std::time::Duration;

use verifier::config::Config;

#[test]
fn example_config_toml_loads() {
    let config = Config::load_from_path("config.toml.example").expect("example config should load");

    assert_eq!(
        config.bind_addr(),
        "127.0.0.1:3000"
            .parse::<SocketAddr>()
            .expect("test bind address should be valid")
    );
    assert_eq!(config.api_key(), None);
    assert_eq!(config.logging_level(), "info");
    assert_eq!(config.network().to_string(), "mainnet");
    assert_eq!(config.toncenter_base_url(), "https://toncenter.com");
    assert_eq!(config.toncenter_api_key(), None);
    assert_eq!(config.source_repository_path(), None);
    assert_eq!(config.source_repository_remote(), "origin");
    assert_eq!(config.source_repository_storage_root(), "sources");
    assert_eq!(config.source_repository_branch(), None);
    assert!(config.source_repository_commit_enabled());
    assert!(config.source_repository_push_enabled());
    assert_eq!(config.source_repository_author_name(), "ton-verifier");
    assert_eq!(
        config.source_repository_author_email(),
        "ton-verifier@example.invalid"
    );
    assert_eq!(
        config.registry_index_path().to_string_lossy(),
        "verifier-index.sqlite3"
    );
    assert_eq!(config.compiler_node_bin(), "node");
    assert_eq!(
        config.compiler_worker_path().to_string_lossy(),
        "compiler-worker/compile.mjs"
    );
    assert_eq!(config.compiler_timeout(), Duration::from_secs(5));
}

#[test]
fn localnet_network_uses_localnet_endpoint_by_default() {
    let mut config_file =
        tempfile::NamedTempFile::new().expect("temporary config file should be created");
    writeln!(
        config_file,
        r#"
[network]
name = "localnet"
"#
    )
    .expect("temporary config should be writable");
    config_file
        .flush()
        .expect("temporary config should be flushed");

    let config = Config::load_from_path(config_file.path()).expect("localnet config should load");

    assert_eq!(config.logging_level(), "info");
    assert_eq!(config.network().to_string(), "localnet");
    assert_eq!(config.toncenter_base_url(), "http://127.0.0.1:5411");
}

#[test]
fn source_repository_config_loads_from_toml() {
    let mut config_file =
        tempfile::NamedTempFile::new().expect("temporary config file should be created");
    writeln!(
        config_file,
        r#"
[server]
api_key = "migration-api-key"

[logging]
level = "debug"

[network]
name = "testnet"

[toncenter]
base_url = "http://127.0.0.1:5412"
api_key = "test-key"

[source_repository]
path = "/tmp/verifier-sources"
remote = "github"
storage_root = "verified/contracts"
branch = "verified-sources"
commit_enabled = false
push_enabled = false
author_name = "Verifier Bot"
author_email = "verifier@example.com"

[registry_index]
path = "/tmp/verifier-index.sqlite3"
"#
    )
    .expect("temporary config should be writable");
    config_file
        .flush()
        .expect("temporary config should be flushed");

    let config = Config::load_from_path(config_file.path()).expect("testnet config should load");

    assert_eq!(config.logging_level(), "debug");
    assert_eq!(config.api_key(), Some("migration-api-key"));
    assert_eq!(config.network().to_string(), "testnet");
    assert_eq!(config.toncenter_base_url(), "http://127.0.0.1:5412");
    assert_eq!(config.toncenter_api_key(), Some("test-key"));
    assert_eq!(
        config
            .source_repository_path()
            .map(|path| path.to_string_lossy()),
        Some("/tmp/verifier-sources".into())
    );
    assert_eq!(config.source_repository_remote(), "github");
    assert_eq!(
        config.source_repository_storage_root(),
        "verified/contracts"
    );
    assert_eq!(config.source_repository_branch(), Some("verified-sources"));
    assert!(!config.source_repository_commit_enabled());
    assert!(!config.source_repository_push_enabled());
    assert_eq!(config.source_repository_author_name(), "Verifier Bot");
    assert_eq!(
        config.source_repository_author_email(),
        "verifier@example.com"
    );
    assert_eq!(
        config.registry_index_path().to_string_lossy(),
        "/tmp/verifier-index.sqlite3"
    );
}
