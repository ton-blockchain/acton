//! Cold copies of public blockchain data for a new, independently keyed node.
//!
//! The source remains locked until copying finishes. A committed seed belongs to
//! the destination and survives interrupted initialization. Installation links
//! only that private seed, never the donor database, and removes the links before
//! the new validator-engine starts writing.

use std::{
    fs::{self, File},
    path::Path,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, ensure};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use crate::{
    bootstrap::acquire_lock,
    storage::{Layout, Manifest, NodeManifest, NodeRole, Settings, TON_RELEASE, write_json_atomic},
    ton::global_config::GlobalConfig,
};

const SEED_DIRECTORY: &str = "node-seed";
const DATABASE_ENTRIES: &[&str] = &[
    "archive",
    "blockdb",
    "catchains",
    "celldb",
    "files",
    "state",
    "static",
];

#[derive(Serialize, Deserialize)]
struct Seed {
    format_version: u32,
    ton_release: String,
    global_config: GlobalConfig,
    entries: Vec<String>,
}

/// Copies a stopped Localton node into an uninitialized destination state directory.
/// The source mount may be read-only; an active Localton owner prevents copying.
/// Only the final directory rename publishes a seed that join may consume.
pub(crate) fn prepare(source: &Path, destination: &Path) -> Result<()> {
    let source = dunce::canonicalize(source)
        .with_context(|| format!("failed to resolve source state {}", source.display()))?;
    fs::create_dir_all(destination)?;
    let destination = dunce::canonicalize(destination)?;
    ensure!(
        !destination.starts_with(&source) && !source.starts_with(&destination),
        "source and destination state directories must be separate"
    );

    let source = Layout::new(source);
    let destination = Layout::new(destination);
    let source_lock = File::open(&source.lock)
        .with_context(|| format!("failed to open source lock {}", source.lock.display()))?;
    FileExt::try_lock_shared(&source_lock)
        .context("stop the source Localton node before copying its blockchain data")?;
    let _destination_lock = acquire_lock(&destination.lock)?;
    ensure!(
        !destination.settings.exists()
            && !destination.manifest.exists()
            && !destination.node.manifest.exists(),
        "blockchain data can only be copied into an uninitialized state directory"
    );

    let settings = Settings::load(&source.settings)?;
    NodeManifest::load(&source.node.manifest, &settings.node.name)?;
    if settings.node.role == NodeRole::Genesis {
        Manifest::load(&source.manifest)?;
    }
    let global_config = GlobalConfig::load(&source.global_config)?;
    global_config.validate_for_node_join()?;
    for required in ["archive", "celldb", "state"] {
        ensure!(
            source.node.db.join(required).is_dir(),
            "source blockchain database is missing {required}: {}",
            source.node.db.display()
        );
    }

    let staging = destination.root.join("node-seed.tmp");
    remove_directory(&staging)?;
    fs::create_dir(&staging)?;
    let mut progress = CopyProgress::new("copy_node_state", &settings.node.name, &destination.root);
    let result = (|| {
        let mut entries = Vec::new();
        for name in DATABASE_ENTRIES {
            let path = source.node.db.join(name);
            match fs::symlink_metadata(&path) {
                Ok(_) => {
                    copy_tree(&path, &staging.join(name), false, &mut progress)?;
                    entries.push((*name).to_owned());
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error).context("failed to inspect source database"),
            }
        }

        write_json_atomic(
            &staging.join("seed.json"),
            &Seed {
                format_version: 1,
                ton_release: TON_RELEASE.to_owned(),
                global_config,
                entries,
            },
        )?;
        let seed = destination.root.join(SEED_DIRECTORY);
        remove_directory(&seed)?;
        fs::rename(&staging, &seed).context("failed to publish copied blockchain data")?;
        Ok(())
    })();
    progress.finish(&result);
    if result.is_err() {
        let _ = remove_directory(&staging);
    }
    result
}

/// Installs a committed seed after fresh node identities have been configured.
/// The caller holds the destination instance lock and must commit the node
/// manifest, then discard the seed, before starting the persistent engine.
pub(super) fn install(layout: &Layout, node: &str) -> Result<()> {
    let source = layout.root.join(SEED_DIRECTORY);
    if !source.try_exists()? {
        return Ok(());
    }
    let seed: Seed = serde_json::from_slice(&fs::read(source.join("seed.json"))?)
        .context("invalid copied blockchain metadata")?;
    ensure!(
        seed.format_version == 1 && seed.ton_release == TON_RELEASE,
        "copied blockchain data has an incompatible format or TON release"
    );
    let config = GlobalConfig::load(&layout.global_config)?;
    ensure!(
        seed.global_config.network_config().validator == config.network_config().validator,
        "copied blockchain data belongs to a different network or hardfork configuration"
    );
    ensure!(
        !seed.entries.is_empty()
            && seed
                .entries
                .iter()
                .all(|name| DATABASE_ENTRIES.contains(&name.as_str())),
        "copied blockchain metadata contains unsupported database entries"
    );

    let staging = layout.node.root.join("copy-import");
    remove_directory(&staging)?;
    fs::create_dir(&staging)?;
    let mut progress = CopyProgress::new("install_node_state", node, &layout.node.db);
    let result = (|| {
        for name in &seed.entries {
            copy_tree(&source.join(name), &staging.join(name), true, &mut progress)?;
        }
        super::database_dump::publish_entries(&staging, &layout.node.db)
    })();
    progress.finish(&result);
    result
}

/// Removes seed links before the initialized node may mutate its database.
/// Retrying after a committed manifest repeats cleanup without importing again.
pub(super) fn discard(layout: &Layout) -> Result<()> {
    remove_directory(&layout.root.join(SEED_DIRECTORY))
}

pub(super) fn exists(layout: &Layout) -> bool {
    layout.root.join(SEED_DIRECTORY).exists()
}

fn copy_tree(
    source: &Path,
    destination: &Path,
    link_staged_files: bool,
    progress: &mut CopyProgress<'_>,
) -> Result<()> {
    let metadata = fs::symlink_metadata(source)?;
    ensure!(
        metadata.is_dir() || metadata.is_file(),
        "blockchain database contains a symlink or special file: {}",
        source.display()
    );
    if metadata.is_dir() {
        fs::create_dir(destination)?;
        for entry in fs::read_dir(source)? {
            let entry = entry?;
            copy_tree(
                &entry.path(),
                &destination.join(entry.file_name()),
                link_staged_files,
                progress,
            )?;
        }
    } else {
        if link_staged_files {
            fs::hard_link(source, destination)
                .with_context(|| format!("failed to install copied data {}", source.display()))?;
        } else {
            // The donor and destination must never share writable database files.
            fs::copy(source, destination)
                .with_context(|| format!("failed to copy blockchain data {}", source.display()))?;
        }
        progress.files += 1;
        progress.bytes += metadata.len();
        if progress.last_report.elapsed() >= Duration::from_secs(5) {
            progress.report("pending");
            progress.last_report = Instant::now();
        }
    }
    Ok(())
}

fn remove_directory(path: &Path) -> Result<()> {
    match fs::remove_dir_all(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error).with_context(|| format!("failed to remove {}", path.display())),
    }
}

struct CopyProgress<'a> {
    operation: &'a str,
    node: &'a str,
    target: &'a Path,
    started: Instant,
    last_report: Instant,
    files: u64,
    bytes: u64,
}

impl<'a> CopyProgress<'a> {
    fn new(operation: &'a str, node: &'a str, target: &'a Path) -> Self {
        let progress = Self {
            operation,
            node,
            target,
            started: Instant::now(),
            last_report: Instant::now(),
            files: 0,
            bytes: 0,
        };
        progress.report("pending");
        progress
    }

    fn report(&self, outcome: &str) {
        info!(
            operation = self.operation,
            node = self.node,
            target = %self.target.display(),
            duration_ms = self.started.elapsed().as_millis(),
            files = self.files,
            bytes = self.bytes,
            outcome,
            "local blockchain data copy"
        );
    }

    fn finish(&self, result: &Result<()>) {
        self.report(if result.is_ok() { "success" } else { "failure" });
        if let Err(error) = result {
            warn!(
                operation = self.operation,
                node = self.node,
                target = %self.target.display(),
                duration_ms = self.started.elapsed().as_millis(),
                outcome = "failure",
                %error,
                "local blockchain data copy failed"
            );
        }
    }
}
