//! Compressed cold snapshots of persistent local TON state.
//!
//! Validator databases preallocate large files, so cloning their Docker volume
//! can consume several gigabytes per snapshot. This module archives only the
//! state required to restart the chain. Logs, downloaded binaries, generated
//! service files, and runtime status are intentionally excluded.

use std::{
    ffi::OsStr,
    fs::{self, File},
    io::{self, BufReader, BufWriter, Write},
    path::{Component, Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use tracing::warn;
use zip::{CompressionMethod, ZipArchive, ZipWriter, write::SimpleFileOptions};

use crate::{
    bootstrap::{acquire_lock, validate_persisted_state},
    cli::{SnapshotArgs, SnapshotCommand},
    storage::{Layout, Manifest, RuntimeState},
    ton::toolchain::absolute_path,
};

const SNAPSHOT_FORMAT_VERSION: u32 = 1;
const SNAPSHOT_ENTRIES: &[&str] = &[
    "dht",
    "genesis",
    "global.config.json",
    "manifest.json",
    "nodes",
    "settings.json",
    "wallets",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SnapshotInfo {
    pub format_version: u32,
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub created_at: u64,
    pub archive_size_bytes: u64,
    pub state_size_bytes: u64,
    pub state_schema_version: u32,
    pub ton_release: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub masterchain_seqno: Option<u32>,
}

pub(crate) fn execute(command: SnapshotCommand) -> Result<()> {
    match command {
        SnapshotCommand::Create { paths, name } => {
            println!("{}", serde_json::to_string_pretty(&create(&paths, name)?)?);
        }
        SnapshotCommand::List { paths } => {
            println!("{}", serde_json::to_string_pretty(&list(&paths)?)?);
        }
        SnapshotCommand::Restore { paths, id } => {
            println!("{}", serde_json::to_string_pretty(&restore(&paths, &id)?)?);
        }
        SnapshotCommand::Delete { paths, id } => {
            delete(&paths, &id)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({ "id": id }))?
            );
        }
    }
    Ok(())
}

fn create(paths: &SnapshotArgs, name: Option<String>) -> Result<SnapshotInfo> {
    let state_dir = existing_state_dir(&paths.state.state_dir)?;
    let snapshot_dir = snapshot_dir(paths, &state_dir, true)?;
    let layout = Layout::new(state_dir.clone());
    let _lock = acquire_lock(&layout.lock)?;
    let manifest = Manifest::load(&layout.manifest)?;
    validate_persisted_state(&layout, &manifest)?;

    let name = normalize_name(name)?;
    let created_at = unix_time();
    let id = allocate_id(&snapshot_dir)?;
    let archive_path = archive_path(&snapshot_dir, &id);
    let temporary_archive = archive_path.with_extension("zip.tmp");
    let state_size_bytes = write_archive(&state_dir, &temporary_archive)?;
    fs::rename(&temporary_archive, &archive_path).with_context(|| {
        format!(
            "failed to commit snapshot archive {}",
            archive_path.display()
        )
    })?;
    let archive_size_bytes = fs::metadata(&archive_path)?.len();
    let masterchain_seqno = RuntimeState::load(&layout.runtime)?.masterchain_seqno;
    let info = SnapshotInfo {
        format_version: SNAPSHOT_FORMAT_VERSION,
        id,
        name,
        created_at,
        archive_size_bytes,
        state_size_bytes,
        state_schema_version: manifest.schema_version,
        ton_release: manifest.ton_release,
        masterchain_seqno,
    };
    if let Err(error) = write_info(&snapshot_dir, &info) {
        let _ = fs::remove_file(&archive_path);
        return Err(error);
    }
    Ok(info)
}

fn list(paths: &SnapshotArgs) -> Result<Vec<SnapshotInfo>> {
    let state_dir = absolute_path(&paths.state.state_dir)?;
    let snapshot_dir = snapshot_dir(paths, &state_dir, false)?;
    if !snapshot_dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut snapshots = Vec::new();
    for entry in fs::read_dir(&snapshot_dir)
        .with_context(|| format!("failed to read {}", snapshot_dir.display()))?
    {
        let path = entry?.path();
        if path.extension() != Some(OsStr::new("json")) {
            continue;
        }
        let info: SnapshotInfo = serde_json::from_slice(
            &fs::read(&path).with_context(|| format!("failed to read {}", path.display()))?,
        )
        .with_context(|| format!("invalid snapshot metadata {}", path.display()))?;
        validate_info(&info)?;
        snapshots.push(info);
    }
    snapshots.sort_by(|left, right| {
        right
            .created_at
            .cmp(&left.created_at)
            .then_with(|| right.id.cmp(&left.id))
    });
    Ok(snapshots)
}

fn restore(paths: &SnapshotArgs, id: &str) -> Result<SnapshotInfo> {
    validate_id(id)?;
    let state_dir = existing_state_dir(&paths.state.state_dir)?;
    let snapshot_dir = snapshot_dir(paths, &state_dir, false)?;
    let info = read_info(&snapshot_dir, id)?;
    let archive_path = archive_path(&snapshot_dir, id);
    ensure!(
        fs::metadata(&archive_path)
            .with_context(|| format!("snapshot archive {} is missing", archive_path.display()))?
            .len()
            == info.archive_size_bytes,
        "snapshot archive size does not match its metadata"
    );

    let layout = Layout::new(state_dir.clone());
    let _lock = acquire_lock(&layout.lock)?;
    let staging = work_dir(&state_dir, "restore", id);
    let backup = work_dir(&state_dir, "backup", id);
    remove_dir_if_exists(&staging)?;
    remove_dir_if_exists(&backup)?;
    fs::create_dir(&staging).with_context(|| format!("failed to create {}", staging.display()))?;

    let result = (|| {
        extract_archive(&archive_path, &staging, info.state_size_bytes)?;
        let staged_layout = Layout::new(staging.clone());
        let mut manifest = Manifest::load(&staged_layout.manifest)?;
        ensure!(
            manifest.schema_version == info.state_schema_version
                && manifest.ton_release == info.ton_release,
            "snapshot state does not match its metadata"
        );
        manifest.global_config.clone_from(&layout.global_config);
        manifest.save_atomic(&staged_layout.manifest)?;
        let mut validation_manifest = manifest.clone();
        validation_manifest
            .global_config
            .clone_from(&staged_layout.global_config);
        validate_persisted_state(&staged_layout, &validation_manifest)?;

        replace_state_entries(&state_dir, &staging, &backup)?;
        let restored_manifest = Manifest::load(&layout.manifest)?;
        if let Err(error) = validate_persisted_state(&layout, &restored_manifest) {
            rollback_state_entries(&state_dir, &staging, &backup)?;
            return Err(error).context("restored snapshot is incomplete");
        }
        RuntimeState::new().save_atomic(&layout.runtime)?;
        Ok(())
    })();

    if result.is_ok()
        && let Err(error) = remove_dir_if_exists(&backup)
    {
        warn!(path = %backup.display(), %error, "failed to remove previous state after snapshot restore");
    }
    if let Err(error) = remove_dir_if_exists(&staging) {
        warn!(path = %staging.display(), %error, "failed to remove snapshot staging directory");
    }
    result?;
    Ok(info)
}

fn delete(paths: &SnapshotArgs, id: &str) -> Result<()> {
    validate_id(id)?;
    let state_dir = absolute_path(&paths.state.state_dir)?;
    let snapshot_dir = snapshot_dir(paths, &state_dir, false)?;
    let info = read_info(&snapshot_dir, id)?;
    let archive = archive_path(&snapshot_dir, &info.id);
    fs::remove_file(&archive).with_context(|| format!("failed to delete {}", archive.display()))?;
    let metadata = info_path(&snapshot_dir, &info.id);
    fs::remove_file(&metadata)
        .with_context(|| format!("failed to delete {}", metadata.display()))?;
    Ok(())
}

fn write_archive(state_dir: &Path, destination: &Path) -> Result<u64> {
    let output = File::create(destination)
        .with_context(|| format!("failed to create {}", destination.display()))?;
    let mut writer = ZipWriter::new(BufWriter::new(output));
    let mut entries = Vec::new();
    for name in SNAPSHOT_ENTRIES {
        let path = state_dir.join(name);
        if path.exists() {
            collect_entries(state_dir, &path, &mut entries)?;
        }
    }
    entries.sort();

    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .compression_level(Some(1));
    let mut state_size_bytes = 0_u64;
    for path in entries {
        let relative = archive_name(state_dir, &path)?;
        let metadata = fs::symlink_metadata(&path)?;
        ensure!(
            !metadata.file_type().is_symlink(),
            "snapshot state contains a symlink: {}",
            path.display()
        );
        if metadata.is_dir() {
            writer.add_directory(format!("{relative}/"), options)?;
            continue;
        }
        ensure!(
            metadata.is_file(),
            "snapshot state contains an unsupported entry: {}",
            path.display()
        );
        writer.start_file(relative, file_options(options, &metadata))?;
        let mut input = BufReader::new(
            File::open(&path).with_context(|| format!("failed to open {}", path.display()))?,
        );
        state_size_bytes = state_size_bytes
            .checked_add(io::copy(&mut input, &mut writer)?)
            .context("snapshot state size overflow")?;
    }
    let mut output = writer.finish()?;
    output.flush()?;
    output.get_ref().sync_all()?;
    Ok(state_size_bytes)
}

fn collect_entries(root: &Path, path: &Path, entries: &mut Vec<PathBuf>) -> Result<()> {
    ensure!(
        path.starts_with(root),
        "snapshot entry is outside the state directory"
    );
    let metadata = fs::symlink_metadata(path)?;
    ensure!(
        !metadata.file_type().is_symlink(),
        "snapshot state contains a symlink: {}",
        path.display()
    );
    entries.push(path.to_owned());
    if !metadata.is_dir() {
        return Ok(());
    }
    let mut children = fs::read_dir(path)?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<io::Result<Vec<_>>>()?;
    children.sort();
    for child in children {
        collect_entries(root, &child, entries)?;
    }
    Ok(())
}

fn extract_archive(archive_path: &Path, destination: &Path, expected_size: u64) -> Result<()> {
    let mut archive = ZipArchive::new(BufReader::new(
        File::open(archive_path)
            .with_context(|| format!("failed to open {}", archive_path.display()))?,
    ))?;
    let mut extracted_size = 0_u64;
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index)?;
        ensure!(!entry.is_symlink(), "snapshot archive contains a symlink");
        let relative = entry
            .enclosed_name()
            .context("snapshot archive contains an unsafe path")?;
        ensure!(
            is_snapshot_entry(&relative),
            "snapshot archive contains an unexpected path: {}",
            relative.display()
        );
        let output = destination.join(&relative);
        ensure!(
            output.starts_with(destination),
            "snapshot archive path escapes its destination"
        );
        if entry.is_dir() {
            fs::create_dir_all(&output)?;
            continue;
        }
        let parent = output.parent().context("snapshot entry has no parent")?;
        fs::create_dir_all(parent)?;
        let mut file = File::create(&output)
            .with_context(|| format!("failed to create {}", output.display()))?;
        extracted_size = extracted_size
            .checked_add(io::copy(&mut entry, &mut file)?)
            .context("snapshot state size overflow")?;
        set_permissions(&output, entry.unix_mode())?;
    }
    ensure!(
        extracted_size == expected_size,
        "snapshot extracted size does not match its metadata"
    );
    Ok(())
}

fn replace_state_entries(state_dir: &Path, staging: &Path, backup: &Path) -> Result<()> {
    fs::create_dir(backup).with_context(|| format!("failed to create {}", backup.display()))?;
    let mut moved = Vec::new();
    for name in SNAPSHOT_ENTRIES {
        let current = state_dir.join(name);
        if current.exists() {
            if let Err(error) = fs::rename(&current, backup.join(name)) {
                for moved_name in moved.iter().rev() {
                    fs::rename(backup.join(moved_name), state_dir.join(moved_name))?;
                }
                return Err(error).with_context(|| {
                    format!("failed to move current state entry {}", current.display())
                });
            }
            moved.push(*name);
        }
    }
    for name in SNAPSHOT_ENTRIES {
        let current = state_dir.join(name);
        let restored = staging.join(name);
        if restored.exists()
            && let Err(error) = fs::rename(&restored, &current)
        {
            rollback_state_entries(state_dir, staging, backup)?;
            return Err(error)
                .with_context(|| format!("failed to restore state entry {}", current.display()));
        }
    }
    Ok(())
}

fn rollback_state_entries(state_dir: &Path, staging: &Path, backup: &Path) -> Result<()> {
    for name in SNAPSHOT_ENTRIES.iter().rev() {
        let current = state_dir.join(name);
        if current.exists() {
            fs::rename(&current, staging.join(name)).with_context(|| {
                format!("failed to roll back restored entry {}", current.display())
            })?;
        }
        let previous = backup.join(name);
        if previous.exists() {
            fs::rename(&previous, &current).with_context(|| {
                format!(
                    "failed to restore previous state entry {}",
                    current.display()
                )
            })?;
        }
    }
    Ok(())
}

fn snapshot_dir(paths: &SnapshotArgs, state_dir: &Path, create: bool) -> Result<PathBuf> {
    let requested = paths
        .snapshot_dir
        .clone()
        .unwrap_or_else(|| default_snapshot_dir(state_dir));
    let requested = absolute_path(&requested)?;
    if create {
        fs::create_dir_all(&requested)
            .with_context(|| format!("failed to create {}", requested.display()))?;
    }
    if !requested.exists() {
        return Ok(requested);
    }
    let state = dunce::canonicalize(state_dir)
        .with_context(|| format!("failed to resolve {}", state_dir.display()))?;
    let snapshots = dunce::canonicalize(&requested)
        .with_context(|| format!("failed to resolve {}", requested.display()))?;
    ensure!(
        !snapshots.starts_with(&state) && !state.starts_with(&snapshots),
        "snapshot directory must be separate from the state directory"
    );
    Ok(snapshots)
}

fn default_snapshot_dir(state_dir: &Path) -> PathBuf {
    let parent = state_dir.parent().unwrap_or_else(|| Path::new("."));
    let name = state_dir
        .file_name()
        .and_then(OsStr::to_str)
        .unwrap_or("localton");
    parent.join(format!("{name}.snapshots"))
}

fn existing_state_dir(path: &Path) -> Result<PathBuf> {
    let path = absolute_path(path)?;
    ensure!(
        path.is_dir(),
        "state directory {} does not exist",
        path.display()
    );
    Ok(path)
}

fn allocate_id(snapshot_dir: &Path) -> Result<String> {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system clock is before the Unix epoch")?
        .as_millis();
    for suffix in 0_u32.. {
        let id = if suffix == 0 {
            format!("snapshot-{millis}")
        } else {
            format!("snapshot-{millis}-{suffix}")
        };
        if !archive_path(snapshot_dir, &id).exists() && !info_path(snapshot_dir, &id).exists() {
            return Ok(id);
        }
    }
    unreachable!("snapshot suffix space is unbounded")
}

fn normalize_name(name: Option<String>) -> Result<Option<String>> {
    let Some(name) = name else {
        return Ok(None);
    };
    let name = name.trim();
    ensure!(!name.is_empty(), "snapshot name must not be empty");
    ensure!(
        name.chars().count() <= 80,
        "snapshot name must not exceed 80 characters"
    );
    Ok(Some(name.to_owned()))
}

fn validate_id(id: &str) -> Result<()> {
    ensure!(
        !id.is_empty()
            && id.len() <= 128
            && id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_')),
        "invalid snapshot id"
    );
    Ok(())
}

fn validate_info(info: &SnapshotInfo) -> Result<()> {
    validate_id(&info.id)?;
    ensure!(
        info.format_version == SNAPSHOT_FORMAT_VERSION,
        "unsupported snapshot format {}",
        info.format_version
    );
    Ok(())
}

fn read_info(snapshot_dir: &Path, id: &str) -> Result<SnapshotInfo> {
    let path = info_path(snapshot_dir, id);
    let info: SnapshotInfo = serde_json::from_slice(
        &fs::read(&path).with_context(|| format!("snapshot {id} does not exist"))?,
    )
    .with_context(|| format!("invalid snapshot metadata {}", path.display()))?;
    validate_info(&info)?;
    ensure!(
        info.id == id,
        "snapshot metadata id does not match its file name"
    );
    Ok(info)
}

fn write_info(snapshot_dir: &Path, info: &SnapshotInfo) -> Result<()> {
    let path = info_path(snapshot_dir, &info.id);
    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, serde_json::to_vec_pretty(info)?)
        .with_context(|| format!("failed to write {}", temporary.display()))?;
    fs::rename(&temporary, &path).with_context(|| format!("failed to commit {}", path.display()))
}

fn archive_path(snapshot_dir: &Path, id: &str) -> PathBuf {
    snapshot_dir.join(format!("{id}.zip"))
}

fn info_path(snapshot_dir: &Path, id: &str) -> PathBuf {
    snapshot_dir.join(format!("{id}.json"))
}

fn work_dir(state_dir: &Path, kind: &str, id: &str) -> PathBuf {
    state_dir.join(format!(".snapshot-{kind}-{id}-{}", std::process::id()))
}

fn remove_dir_if_exists(path: &Path) -> Result<()> {
    match fs::remove_dir_all(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error).with_context(|| format!("failed to remove {}", path.display())),
    }
}

fn archive_name(root: &Path, path: &Path) -> Result<String> {
    let relative = path.strip_prefix(root)?;
    let mut parts = Vec::new();
    for component in relative.components() {
        let Component::Normal(component) = component else {
            bail!("snapshot path contains an unsupported component")
        };
        parts.push(
            component
                .to_str()
                .context("snapshot path is not valid UTF-8")?,
        );
    }
    Ok(parts.join("/"))
}

fn is_snapshot_entry(path: &Path) -> bool {
    let Some(Component::Normal(first)) = path.components().next() else {
        return false;
    };
    SNAPSHOT_ENTRIES
        .iter()
        .any(|allowed| first == OsStr::new(allowed))
}

fn unix_time() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(unix)]
fn file_options(options: SimpleFileOptions, metadata: &fs::Metadata) -> SimpleFileOptions {
    use std::os::unix::fs::PermissionsExt;
    options.unix_permissions(metadata.permissions().mode())
}

#[cfg(not(unix))]
fn file_options(options: SimpleFileOptions, _metadata: &fs::Metadata) -> SimpleFileOptions {
    options
}

#[cfg(unix)]
fn set_permissions(path: &Path, mode: Option<u32>) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    if let Some(mode) = mode {
        fs::set_permissions(path, fs::Permissions::from_mode(mode & 0o777))?;
    }
    Ok(())
}

#[cfg(not(unix))]
fn set_permissions(_path: &Path, _mode: Option<u32>) -> Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use expect_test::expect;
    use tempfile::TempDir;

    use super::*;
    use crate::{
        cli::StateArgs,
        storage::{SCHEMA_VERSION, TON_RELEASE},
    };

    #[test]
    fn snapshot_round_trip_excludes_rebuildable_data() {
        let fixture = Fixture::new();
        fs::write(
            fixture.layout.logs.join("large.log"),
            vec![b'x'; 2 * 1024 * 1024],
        )
        .unwrap();
        fs::write(fixture.layout.validator_db.join("marker"), b"before").unwrap();

        let created = create(&fixture.paths, Some("Before upgrade".to_owned())).unwrap();
        fs::write(fixture.layout.validator_db.join("marker"), b"after").unwrap();
        let restored = restore(&fixture.paths, &created.id).unwrap();
        let listed = list(&fixture.paths).unwrap();
        let actual = format!(
            "name: {:?}\narchive smaller than excluded log: {}\nrestored marker: {}\nlisted snapshots: {}\nruntime seqno after restore: {:?}\nrestored same snapshot: {}",
            created.name,
            created.archive_size_bytes < 2 * 1024 * 1024,
            fs::read_to_string(fixture.layout.validator_db.join("marker")).unwrap(),
            listed.len(),
            RuntimeState::load(&fixture.layout.runtime)
                .unwrap()
                .masterchain_seqno,
            restored.id == created.id,
        );

        expect![[r#"name: Some("Before upgrade")
archive smaller than excluded log: true
restored marker: before
listed snapshots: 1
runtime seqno after restore: None
restored same snapshot: true"#]]
        .assert_eq(&actual);
    }

    #[test]
    fn snapshot_create_refuses_a_live_state_directory() {
        let fixture = Fixture::new();
        let _lock = acquire_lock(&fixture.layout.lock).unwrap();
        let error = create(&fixture.paths, None).unwrap_err().to_string();
        let actual = format!(
            "lock rejected: {}\nmessage identifies a live localton process: {}",
            !error.is_empty(),
            error.contains("another localton process is already using"),
        );

        expect![[r"lock rejected: true
message identifies a live localton process: true"]]
        .assert_eq(&actual);
    }

    #[test]
    fn snapshot_directory_cannot_be_inside_state() {
        let mut fixture = Fixture::new();
        fixture.paths.snapshot_dir = Some(fixture.state_dir.join("snapshots"));
        let error = create(&fixture.paths, None).unwrap_err().to_string();

        expect!["snapshot directory must be separate from the state directory"].assert_eq(&error);
    }

    #[test]
    fn restore_work_directory_uses_the_state_filesystem() {
        let state_dir = Path::new("/var/lib/localton");
        let work = work_dir(state_dir, "restore", "snapshot-1");
        let actual = format!(
            "inside state: {}\nhidden restore directory: {}",
            work.parent() == Some(state_dir),
            work.file_name()
                .and_then(OsStr::to_str)
                .is_some_and(|name| name.starts_with(".snapshot-restore-snapshot-1-")),
        );

        expect![[r#"inside state: true
hidden restore directory: true"#]]
        .assert_eq(&actual);
    }

    struct Fixture {
        _root: TempDir,
        state_dir: PathBuf,
        layout: Layout,
        paths: SnapshotArgs,
    }

    impl Fixture {
        fn new() -> Self {
            let root = tempfile::tempdir_in("/tmp").unwrap();
            let state_dir = root.path().join("localton");
            let snapshot_dir = root.path().join("snapshots");
            let layout = Layout::new(state_dir.clone());
            layout.create_dirs().unwrap();
            fs::write(&layout.global_config, "{}\n").unwrap();
            fs::write(layout.validator_db.join("config.json"), "{}\n").unwrap();
            fs::write(layout.dht_db.join("config.json"), "{}\n").unwrap();
            fs::write(layout.certs.join("client"), b"client").unwrap();
            fs::write(layout.certs.join("server.pub"), b"server").unwrap();
            fs::write(&layout.settings, "{}\n").unwrap();
            Manifest {
                schema_version: SCHEMA_VERSION,
                ton_release: TON_RELEASE.to_owned(),
                ton_bin_dir: None,
                validator_id_hex: "11".repeat(32),
                validator_id_base64: "validator".to_owned(),
                liteserver_public_key: "liteserver".to_owned(),
                global_config: layout.global_config.clone(),
                imported_accounts: Vec::new(),
            }
            .save_atomic(&layout.manifest)
            .unwrap();
            let mut runtime = RuntimeState::new();
            runtime.masterchain_seqno = Some(42);
            runtime.save_atomic(&layout.runtime).unwrap();
            let paths = SnapshotArgs {
                state: StateArgs {
                    state_dir: state_dir.clone(),
                },
                snapshot_dir: Some(snapshot_dir),
            };
            Self {
                _root: root,
                state_dir,
                layout,
                paths,
            }
        }
    }
}
