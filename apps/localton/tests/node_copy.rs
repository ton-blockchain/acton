//! CLI coverage of cold-copy ownership, publication, and source independence.

use std::{
    fs,
    path::Path,
    process::{Command, Output},
};

use anyhow::{Result, ensure};
use base64::{Engine, engine::general_purpose::STANDARD};
use expect_test::expect;
use fs2::FileExt;
use serde_json::json;

fn copy(source: &Path, destination: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_localton"))
        .args(["node", "copy-state", "--from-state-dir"])
        .arg(source)
        .arg("--state-dir")
        .arg(destination)
        .output()
        .expect("run Localton")
}

fn source(root: &Path) -> Result<()> {
    for directory in ["archive", "celldb", "state", "keyring"] {
        fs::create_dir_all(root.join("node/db").join(directory))?;
    }
    fs::write(root.join("instance.lock"), "")?;
    fs::write(
        root.join("settings.json"),
        r#"{"node":{"name":"donor","role":"joined"}}"#,
    )?;
    fs::write(
        root.join("global.config.json"),
        include_bytes!(
            "../../../crates/ton-indexer-liteserver/fixtures/mainnet-global.config.json"
        ),
    )?;
    fs::write(
        root.join("node/node-manifest.json"),
        serde_json::to_vec(&json!({
            "schema_version": 1,
            "node_name": "donor",
            "console_public_key": STANDARD.encode([6; 32]),
            "liteserver_public_key": STANDARD.encode([2; 32]),
            "full_node_adnl": STANDARD.encode([3; 32]),
            "validator_public_key": null,
            "validator_adnl": null,
        }))?,
    )?;
    fs::write(root.join("node/db/archive/block"), "block data")?;
    fs::write(root.join("node/db/celldb/cells"), "cell data")?;
    fs::write(root.join("node/db/state/head"), "42")?;
    fs::write(
        root.join("node/db/keyring/private-key"),
        "private fixture key",
    )?;
    fs::write(
        root.join("node/db/config.json"),
        "private node configuration",
    )?;
    fs::write(
        root.join("node/db/validators.suspended.json"),
        "node-local policy",
    )?;
    Ok(())
}

#[test]
fn cold_copy_preserves_source_and_excludes_node_identity() -> Result<()> {
    let root = tempfile::tempdir_in("/tmp")?;
    let donor = root.path().join("donor");
    let destination = root.path().join("new-node");
    source(&donor)?;

    let lock = fs::File::open(donor.join("instance.lock"))?;
    FileExt::try_lock_exclusive(&lock)?;
    let busy = copy(&donor, &destination);
    drop(lock);

    let result = copy(&donor, &destination);
    ensure!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let seed = destination.join("node-seed");
    let mut copied = fs::read_dir(&seed)?
        .map(|entry| entry.map(|entry| entry.file_name().to_string_lossy().into_owned()))
        .collect::<std::io::Result<Vec<_>>>()?;
    copied.sort();

    fs::write(seed.join("archive/block"), "changed destination")?;
    fs::write(destination.join("settings.json"), "{}")?;
    let initialized = copy(&donor, &destination);
    let same_directory = copy(&donor, &donor);

    expect![[r#"
        {
          "copied": [
            "archive",
            "celldb",
            "seed.json",
            "state"
          ],
          "initializedTargetRejected": true,
          "liveSourceRejected": true,
          "sameDirectoryRejected": true,
          "sourceBlock": "block data",
          "sourceKey": "private fixture key",
          "stagingRemoved": true
        }"#]].assert_eq(&serde_json::to_string_pretty(&json!({
        "copied": copied,
        "initializedTargetRejected": !initialized.status.success()
            && String::from_utf8_lossy(&initialized.stderr).contains("uninitialized state directory"),
        "liveSourceRejected": !busy.status.success()
            && String::from_utf8_lossy(&busy.stderr).contains("stop the source Localton node"),
        "sameDirectoryRejected": !same_directory.status.success()
            && String::from_utf8_lossy(&same_directory.stderr).contains("must be separate"),
        "sourceBlock": fs::read_to_string(donor.join("node/db/archive/block"))?,
        "sourceKey": fs::read_to_string(donor.join("node/db/keyring/private-key"))?,
        "stagingRemoved": !destination.join("node-seed.tmp").exists(),
    }))?);
    Ok(())
}

#[cfg(unix)]
#[test]
fn failed_copy_does_not_publish_partial_data() -> Result<()> {
    let root = tempfile::tempdir_in("/tmp")?;
    let donor = root.path().join("donor");
    let destination = root.path().join("new-node");
    source(&donor)?;
    std::os::unix::fs::symlink("../keyring/private-key", donor.join("node/db/archive/link"))?;
    let result = copy(&donor, &destination);

    expect![[r#"
        {
          "seedPublished": false,
          "stagingRemoved": true,
          "symlinkRejected": true
        }"#]]
    .assert_eq(&serde_json::to_string_pretty(&json!({
        "seedPublished": destination.join("node-seed").exists(),
        "stagingRemoved": !destination.join("node-seed.tmp").exists(),
        "symlinkRejected": !result.status.success()
            && String::from_utf8_lossy(&result.stderr).contains("symlink or special file"),
    }))?);
    Ok(())
}
