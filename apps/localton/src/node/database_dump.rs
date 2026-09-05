//! Safe import of official TON validator database dumps.
//!
//! Dumps contain public chain data only. Localton creates the node configuration
//! and private keyring first, extracts the dump into a sibling staging directory,
//! and publishes only database-owned entries after extraction succeeds.

use std::{
    ffi::OsStr,
    fs::{self, File},
    io::Read,
    path::{Component, Path},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

use anyhow::{Context, Result, bail, ensure};
use tracing::{info, warn};

const PROTECTED_ENTRIES: &[&str] = &[
    "config.json",
    "global.config.json",
    "keyring",
    "ton-global.config",
];
const DATABASE_MARKERS: &[&str] = &["archive", "blockdb", "celldb", "files", "state"];
const PROGRESS_INTERVAL: Duration = Duration::from_secs(5);

/// Imports a dump without replacing Localton's node identity or configuration.
///
/// The operation is first-initialization-only at the caller. Extraction is
/// blocking and therefore runs outside the async executor. A failed or interrupted
/// import never writes the node manifest, so the next join attempt can discard the
/// partial node tree and retry from the original archive.
pub(super) async fn import(archive: &Path, database: &Path, node: &str) -> Result<()> {
    ensure!(
        archive.is_file(),
        "TON database dump does not exist: {}",
        archive.display()
    );
    let archive_size = fs::metadata(archive)?.len();
    let staging = database
        .parent()
        .context("validator database has no parent directory")?
        .join("dump-import");
    remove_directory(&staging)?;
    fs::create_dir(&staging).with_context(|| {
        format!(
            "failed to create dump staging directory {}",
            staging.display()
        )
    })?;

    let started = Instant::now();
    info!(
        operation = "import_database_dump",
        node,
        target = %archive.display(),
        archive_size_bytes = archive_size,
        outcome = "pending",
        "importing TON validator database dump"
    );

    let archive_for_task = archive.to_owned();
    let database = database.to_owned();
    let staging_for_task = staging.clone();
    let node_for_task = node.to_owned();
    let result = tokio::task::spawn_blocking(move || {
        import_blocking(
            &archive_for_task,
            &database,
            &staging_for_task,
            &node_for_task,
        )
    })
    .await
    .context("TON database dump import task failed")
    .and_then(|result| result);

    match &result {
        Ok(()) => info!(
            operation = "import_database_dump",
            node,
            target = %archive.display(),
            duration_ms = started.elapsed().as_millis(),
            outcome = "success",
            "TON validator database dump imported"
        ),
        Err(error) => warn!(
            operation = "import_database_dump",
            node,
            target = %archive.display(),
            duration_ms = started.elapsed().as_millis(),
            outcome = "failure",
            %error,
            "TON validator database dump import failed"
        ),
    }

    if result.is_err() {
        let _ = remove_directory(&staging);
    }
    result
}

fn import_blocking(archive: &Path, database: &Path, staging: &Path, node: &str) -> Result<()> {
    match archive.extension().and_then(OsStr::to_str) {
        Some("lz") => extract_lzip_archive(archive, staging, node)?,
        Some("tar") => extract_tar(File::open(archive)?, staging, node)?,
        _ => bail!("TON database dump must have a .tar.lz or .tar extension"),
    }
    ensure_dump_layout(staging)?;
    publish_entries(staging, database)
}

fn extract_lzip_archive(archive: &Path, destination: &Path, node: &str) -> Result<()> {
    let mut child = spawn_lzip(archive)?;
    let stdout = child
        .stdout
        .take()
        .context("lzip decompressor did not expose stdout")?;
    let extract_result = extract_tar(stdout, destination, node);
    let status = child
        .wait()
        .context("failed to wait for lzip decompressor")?;
    extract_result?;
    ensure!(
        status.success(),
        "lzip decompression failed with status {status}"
    );
    Ok(())
}

fn spawn_lzip(archive: &Path) -> Result<std::process::Child> {
    let mut missing = Vec::new();
    for executable in ["plzip", "lzip"] {
        match Command::new(executable)
            .args(["-d", "-c", "--"])
            .arg(archive)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
        {
            Ok(child) => return Ok(child),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => missing.push(executable),
            Err(error) => {
                return Err(error).with_context(|| format!("failed to start {executable}"));
            }
        }
    }
    bail!(
        "cannot import .tar.lz: install plzip or lzip and make it available on PATH ({})",
        missing.join(", ")
    )
}

fn extract_tar(reader: impl Read, destination: &Path, node: &str) -> Result<()> {
    let mut archive = tar::Archive::new(reader);
    let mut entries = 0_u64;
    let mut extracted_bytes = 0_u64;
    let mut last_progress = Instant::now();
    for entry in archive
        .entries()
        .context("failed to read TON dump tar entries")?
    {
        let mut entry = entry.context("failed to read a TON dump tar entry")?;
        let path = entry
            .path()
            .context("failed to decode a TON dump path")?
            .into_owned();
        let top_level = safe_top_level(&path)?;
        ensure!(
            !PROTECTED_ENTRIES
                .iter()
                .any(|protected| top_level == OsStr::new(protected)),
            "TON dump attempts to replace protected node entry {}",
            top_level.to_string_lossy()
        );
        let entry_type = entry.header().entry_type();
        ensure!(
            entry_type.is_file() || entry_type.is_dir(),
            "TON dump contains unsupported entry type at {}",
            path.display()
        );
        entries += 1;
        extracted_bytes = extracted_bytes.saturating_add(entry.header().size().unwrap_or(0));
        ensure!(
            entry.unpack_in(destination)?,
            "TON dump path escapes its staging directory: {}",
            path.display()
        );
        if last_progress.elapsed() >= PROGRESS_INTERVAL {
            info!(
                operation = "import_database_dump",
                node,
                target = %destination.display(),
                entries,
                extracted_bytes,
                outcome = "pending",
                "extracting TON validator database dump"
            );
            last_progress = Instant::now();
        }
    }
    Ok(())
}

fn safe_top_level(path: &Path) -> Result<&OsStr> {
    let mut top_level = None;
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::Normal(value) => {
                top_level.get_or_insert(value);
            }
            _ => bail!("TON dump contains an unsafe path: {}", path.display()),
        };
    }
    top_level.with_context(|| format!("TON dump contains an empty path: {}", path.display()))
}

fn ensure_dump_layout(staging: &Path) -> Result<()> {
    let has_database = DATABASE_MARKERS
        .iter()
        .any(|entry| staging.join(entry).exists());
    ensure!(
        has_database,
        "archive does not contain a TON validator database (expected one of {})",
        DATABASE_MARKERS.join(", ")
    );
    Ok(())
}

fn publish_entries(staging: &Path, database: &Path) -> Result<()> {
    for entry in fs::read_dir(staging)? {
        let entry = entry?;
        let destination = database.join(entry.file_name());
        remove_path(&destination)?;
        fs::rename(entry.path(), &destination).with_context(|| {
            format!("failed to publish TON dump entry {}", destination.display())
        })?;
    }
    remove_directory(staging)
}

fn remove_path(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_dir() => fs::remove_dir_all(path),
        Ok(_) => fs::remove_file(path),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(error).with_context(|| format!("failed to inspect {}", path.display()));
        }
    }
    .with_context(|| {
        format!(
            "failed to replace existing database entry {}",
            path.display()
        )
    })
}

fn remove_directory(path: &Path) -> Result<()> {
    match fs::remove_dir_all(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error).with_context(|| format!("failed to remove {}", path.display())),
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use expect_test::expect;

    use super::*;

    #[test]
    fn dump_import_preserves_local_identity_and_publishes_chain_data() {
        let root = tempfile::tempdir_in("/tmp").unwrap();
        let database = root.path().join("node/db");
        let staging = root.path().join("node/dump-import");
        fs::create_dir_all(database.join("keyring")).unwrap();
        fs::write(database.join("config.json"), "local-config").unwrap();
        fs::write(database.join("keyring/local-key"), "secret").unwrap();
        fs::create_dir(database.join("archive")).unwrap();
        fs::write(database.join("archive/old-package"), "stale").unwrap();
        fs::create_dir_all(&staging).unwrap();

        extract_tar(
            Cursor::new(test_archive(&[("archive/package", b"blocks")])),
            &staging,
            "test-node",
        )
        .unwrap();
        ensure_dump_layout(&staging).unwrap();
        publish_entries(&staging, &database).unwrap();

        let actual = format!(
            "config={}\nkey={}\narchive={}\nstaging_exists={}\nstale_archive_exists={}",
            fs::read_to_string(database.join("config.json")).unwrap(),
            fs::read_to_string(database.join("keyring/local-key")).unwrap(),
            fs::read_to_string(database.join("archive/package")).unwrap(),
            staging.exists(),
            database.join("archive/old-package").exists()
        );
        expect![[r#"
            config=local-config
            key=secret
            archive=blocks
            staging_exists=false
            stale_archive_exists=false"#]]
        .assert_eq(&actual);
    }

    #[test]
    fn dump_cannot_replace_local_configuration() {
        let root = tempfile::tempdir_in("/tmp").unwrap();
        let staging = root.path().join("staging");
        fs::create_dir(&staging).unwrap();
        let error = extract_tar(
            Cursor::new(test_archive(&[("config.json", b"foreign-config")])),
            &staging,
            "test-node",
        )
        .unwrap_err();

        expect!["TON dump attempts to replace protected node entry config.json"]
            .assert_eq(&error.to_string());
    }

    fn test_archive(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut bytes = Vec::new();
        {
            let mut builder = tar::Builder::new(&mut bytes);
            for (path, contents) in entries {
                let mut header = tar::Header::new_gnu();
                header.set_size(contents.len() as u64);
                header.set_mode(0o600);
                header.set_cksum();
                builder.append_data(&mut header, path, *contents).unwrap();
            }
            builder.finish().unwrap();
        }
        bytes
    }
}
