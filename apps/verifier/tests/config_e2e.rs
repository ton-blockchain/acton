use std::io::Write;
use std::net::SocketAddr;
use std::time::Duration;

use verifier::config::{Config, TonNetwork};
use verifier::payment::{OnchainPaymentVerifier, PaymentVerifier};

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
    assert!(!config.read_only());
    assert_eq!(config.logging_level(), "info");
    assert_eq!(config.toncenter_mainnet_base_url(), "https://toncenter.com");
    assert_eq!(config.toncenter_mainnet_api_key(), None);
    assert_eq!(
        config.toncenter_testnet_base_url(),
        "https://testnet.toncenter.com"
    );
    assert_eq!(config.toncenter_testnet_api_key(), None);
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
    assert_eq!(config.payment_address(), None);
    assert_eq!(config.payment_min_amount_nano(), None);
    assert_eq!(config.payment_primary_network().to_string(), "testnet");
    assert_eq!(
        config.payment_ledger_path().to_string_lossy(),
        "verifier-payments.sqlite3"
    );
    assert_eq!(config.compiler_node_bin(), "node");
    assert_eq!(
        config.compiler_worker_path().to_string_lossy(),
        "compiler-worker/compile.mjs"
    );
    assert_eq!(config.compiler_timeout(), Duration::from_secs(10));
    assert_eq!(config.max_concurrent_compilations(), Some(1));
    assert_eq!(config.max_request_bytes(), 512 * 1024);
}

#[test]
fn omitted_payment_primary_network_uses_testnet() {
    let mut config_file =
        tempfile::NamedTempFile::new().expect("temporary config file should be created");
    writeln!(config_file, "[logging]\nlevel = \"debug\"")
        .expect("temporary config should be writable");
    config_file
        .flush()
        .expect("temporary config should be flushed");

    let config = Config::load_from_path(config_file.path()).expect("default config should load");

    assert_eq!(config.logging_level(), "debug");
    assert!(!config.read_only());
    assert_eq!(config.toncenter_mainnet_base_url(), "https://toncenter.com");
    assert_eq!(
        config.toncenter_testnet_base_url(),
        "https://testnet.toncenter.com"
    );
    assert_eq!(config.payment_primary_network().to_string(), "testnet");
    assert_eq!(config.compiler_timeout(), Duration::from_secs(10));
    assert_eq!(config.max_concurrent_compilations(), Some(1));
    assert_eq!(config.max_request_bytes(), 512 * 1024);
    assert_eq!(
        Config::default().compiler_timeout(),
        Duration::from_secs(10)
    );
}

#[test]
fn compiler_timeout_can_be_overridden() {
    let mut config_file = tempfile::NamedTempFile::new().expect("config file");
    writeln!(
        config_file,
        "[compiler]\ntimeout_ms = 15000\nmax_concurrent_compilations = 3"
    )
    .expect("write config");
    let config = Config::load_from_path(config_file.path()).expect("custom compiler config");
    assert_eq!(config.compiler_timeout(), Duration::from_secs(15));
    assert_eq!(config.max_concurrent_compilations(), Some(3));
}

#[test]
fn minus_one_disables_the_compiler_concurrency_limit() {
    let mut config_file = tempfile::NamedTempFile::new().expect("config file");
    writeln!(config_file, "[compiler]\nmax_concurrent_compilations = -1").expect("write config");

    let config = Config::load_from_path(config_file.path()).expect("unlimited concurrency");
    assert_eq!(config.max_concurrent_compilations(), None);
}

#[test]
fn invalid_compiler_concurrency_is_rejected() {
    for value in [0, -2] {
        let mut config_file = tempfile::NamedTempFile::new().expect("config file");
        writeln!(
            config_file,
            "[compiler]\nmax_concurrent_compilations = {value}"
        )
        .expect("write config");

        let error = Config::load_from_path(config_file.path()).expect_err("invalid limit");
        assert_eq!(
            error.to_string(),
            format!(
                "compiler max_concurrent_compilations must be -1 or a positive integer, got {value}"
            )
        );
    }
}

#[test]
fn upload_request_limit_can_be_overridden() {
    let mut config_file = tempfile::NamedTempFile::new().expect("config file");
    writeln!(
        config_file,
        r"
[upload_limits]
max_request_bytes = 1000
"
    )
    .expect("write config");

    let config = Config::load_from_path(config_file.path()).expect("custom upload limits");
    assert_eq!(config.max_request_bytes(), 1000);
}

#[test]
fn docker_entrypoint_generates_default_and_overridden_compiler_settings() {
    for (override_ms, concurrency, expected_timeout, expected_concurrency) in [
        (None, None, 10, Some(1)),
        (Some("15000"), Some("3"), 15, Some(3)),
        (None, Some("-1"), 10, None),
    ] {
        let directory = tempfile::tempdir().expect("config directory");
        let config_path = directory.path().join("config.toml");
        let mut command = std::process::Command::new("sh");
        command
            .args(["docker/entrypoint.sh", "true"])
            .env_clear()
            .env("PATH", std::env::var_os("PATH").expect("PATH"))
            .env("VERIFIER_CONFIG", &config_path);
        if let Some(value) = override_ms {
            command.env("VERIFIER_COMPILER_TIMEOUT_MS", value);
        }
        if let Some(value) = concurrency {
            command.env("VERIFIER_COMPILER_MAX_CONCURRENT_COMPILATIONS", value);
        }
        let output = command.output().expect("run entrypoint");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let config = Config::load_from_path(&config_path).expect("generated config");
        assert_eq!(
            config.compiler_timeout(),
            Duration::from_secs(expected_timeout)
        );
        assert_eq!(config.max_concurrent_compilations(), expected_concurrency);
    }
}

#[test]
fn docker_entrypoint_generates_upload_request_limit() {
    let directory = tempfile::tempdir().expect("config directory");
    let config_path = directory.path().join("config.toml");
    let output = std::process::Command::new("sh")
        .args(["docker/entrypoint.sh", "true"])
        .env_clear()
        .env("PATH", std::env::var_os("PATH").expect("PATH"))
        .env("VERIFIER_CONFIG", &config_path)
        .env("VERIFIER_UPLOAD_MAX_REQUEST_BYTES", "1000")
        .output()
        .expect("run entrypoint");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let config = Config::load_from_path(&config_path).expect("generated config");
    assert_eq!(config.max_request_bytes(), 1000);
}

#[test]
fn docker_entrypoint_generates_read_only_mode() {
    let directory = tempfile::tempdir().expect("config directory");
    let config_path = directory.path().join("config.toml");
    let output = std::process::Command::new("sh")
        .args(["docker/entrypoint.sh", "true"])
        .env_clear()
        .env("PATH", std::env::var_os("PATH").expect("PATH"))
        .env("VERIFIER_CONFIG", &config_path)
        .env("VERIFIER_READ_ONLY", "true")
        .output()
        .expect("run entrypoint");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let config = Config::load_from_path(&config_path).expect("generated config");
    assert!(config.read_only());
}

#[test]
fn docker_entrypoint_generates_toncenter_and_payment_settings() {
    let directory = tempfile::tempdir().expect("config directory");
    let config_path = directory.path().join("config.toml");
    let output = std::process::Command::new("sh")
        .args(["docker/entrypoint.sh", "true"])
        .env_clear()
        .env("PATH", std::env::var_os("PATH").expect("PATH"))
        .env("VERIFIER_CONFIG", &config_path)
        .env("VERIFIER_PAYMENT_PRIMARY_NETWORK", "mainnet")
        .env(
            "VERIFIER_TONCENTER_MAINNET_BASE_URL",
            "https://mainnet.example.com",
        )
        .env("VERIFIER_TONCENTER_MAINNET_API_KEY", "mainnet-key")
        .env(
            "VERIFIER_TONCENTER_TESTNET_BASE_URL",
            "https://testnet.example.com",
        )
        .env("VERIFIER_TONCENTER_TESTNET_API_KEY", "testnet-key")
        .output()
        .expect("run entrypoint");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let config = Config::load_from_path(&config_path).expect("generated config");
    assert_eq!(
        config.toncenter_mainnet_base_url(),
        "https://mainnet.example.com"
    );
    assert_eq!(config.toncenter_mainnet_api_key(), Some("mainnet-key"));
    assert_eq!(
        config.toncenter_testnet_base_url(),
        "https://testnet.example.com"
    );
    assert_eq!(config.toncenter_testnet_api_key(), Some("testnet-key"));
    assert_eq!(config.payment_primary_network(), TonNetwork::Mainnet);
}

#[test]
fn mainnet_payment_network_is_supported() {
    let directory = tempfile::tempdir().expect("temporary directory should be created");
    let config_path = directory.path().join("config.toml");
    let ledger_path = directory.path().join("payments.sqlite3");
    std::fs::write(
        &config_path,
        format!(
            r#"
[payment]
primary_network = "mainnet"
address = "0:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
min_amount_nano = 10000000
ledger_path = "{}"
"#,
            ledger_path.display()
        ),
    )
    .expect("temporary config should be writable");

    let config = Config::load_from_path(&config_path).expect("mainnet config should parse");
    let verifier =
        OnchainPaymentVerifier::from_config(&config).expect("mainnet payments should be supported");
    assert_eq!(
        verifier
            .quote("a873d8c2d163f7fa10bbe38769706f0554505e8ea2dcea3f115288db8becf2ab")
            .network,
        TonNetwork::Mainnet
    );
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
read_only = true

[logging]
level = "debug"

[toncenter]
mainnet_base_url = "http://127.0.0.1:5413"
mainnet_api_key = "mainnet-key"
testnet_base_url = "http://127.0.0.1:5412"
testnet_api_key = "test-key"

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

[payment]
primary_network = "testnet"
address = "0:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
min_amount_nano = 10000000
ledger_path = "/tmp/verifier-payments.sqlite3"
"#
    )
    .expect("temporary config should be writable");
    config_file
        .flush()
        .expect("temporary config should be flushed");

    let config = Config::load_from_path(config_file.path()).expect("config should load");

    assert_eq!(config.logging_level(), "debug");
    assert_eq!(config.api_key(), Some("migration-api-key"));
    assert!(config.read_only());
    assert_eq!(config.toncenter_mainnet_base_url(), "http://127.0.0.1:5413");
    assert_eq!(config.toncenter_mainnet_api_key(), Some("mainnet-key"));
    assert_eq!(config.toncenter_testnet_base_url(), "http://127.0.0.1:5412");
    assert_eq!(config.toncenter_testnet_api_key(), Some("test-key"));
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
    assert_eq!(
        config.payment_address(),
        Some("0:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef")
    );
    assert_eq!(config.payment_min_amount_nano(), Some(10_000_000));
    assert_eq!(config.payment_primary_network().to_string(), "testnet");
    assert_eq!(
        config.payment_ledger_path().to_string_lossy(),
        "/tmp/verifier-payments.sqlite3"
    );
}
