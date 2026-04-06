use crate::build_info;
use crate::paths;
use crate::stdlib;
use acton_config::color::OwoColorize;
use acton_config::config::{
    ActonConfig, LibrariesFile, WalletsFile, global_libraries_path, global_wallets_path,
    resolved_paths_diagnostics,
};
use anyhow::Result;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::any::Any;
use std::collections::{BTreeMap, BTreeSet};
use std::fs::OpenOptions;
use std::io::Write as _;
use std::panic::AssertUnwindSafe;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use std::{env, fs};

const DOCTOR_API_CONNECT_TIMEOUT: Duration = Duration::from_secs(4);
const DOCTOR_API_REQUEST_TIMEOUT: Duration = Duration::from_secs(8);
const DOCTOR_TONCENTER_REQUEST_STAGGER: Duration = Duration::from_secs(1);
const DOCTOR_API_TARGETS_JSON_ENV: &str = "ACTON_DOCTOR_API_TARGETS_JSON";
const DEFAULT_DTON_API_KEY: &str = "fpYxhGTWfIe3ZEf2s6vvgAGmps_qnNmD";

#[derive(Debug, Serialize)]
struct DoctorPath {
    path: String,
    exists: bool,
    writable: bool,
    canonical_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    size_bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    size_human: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    resolution_source: Option<String>,
}

#[derive(Debug, Serialize)]
struct DoctorVersions {
    acton: String,
    release_channel: String,
    git_sha: String,
    build_date: String,
    target_triple: String,
    profile: String,
    os: String,
    arch: String,
}

#[derive(Debug, Serialize)]
struct DoctorManifest {
    exists: bool,
    parse_ok: bool,
    error: Option<String>,
    contracts_count: Option<usize>,
    scripts_count: Option<usize>,
    mappings_count: Option<usize>,
}

#[derive(Debug, Serialize)]
struct DoctorPaths {
    project_root: DoctorPath,
    manifest_path: DoctorPath,
    acton_dir: DoctorPath,
    cache_dir: DoctorPath,
    wallets: DoctorPath,
    global_wallets: Option<DoctorPath>,
    libraries: DoctorPath,
    global_libraries: Option<DoctorPath>,
}

#[derive(Debug, Serialize)]
struct DoctorOverlayFile {
    path: DoctorPath,
    #[serde(skip_serializing_if = "Option::is_none")]
    parse_ok: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    entries_count: Option<usize>,
}

#[derive(Debug, Serialize)]
struct DoctorOverlayConfig {
    load_ok: bool,
    merged_entries_count: Option<usize>,
    local: DoctorOverlayFile,
    global: Option<DoctorOverlayFile>,
}

#[derive(Debug, Serialize)]
struct DoctorConfigOverlays {
    wallets: DoctorOverlayConfig,
    libraries: DoctorOverlayConfig,
}

#[derive(Debug, Serialize)]
struct DoctorStdlib {
    path: DoctorPath,
    common_tolk: DoctorPath,
    status: String,
    version: Option<String>,
    expected_version: String,
    revision: Option<String>,
    source: String,
}

#[derive(Debug, Serialize)]
struct DoctorLogging {
    acton_log_dir_env: Option<String>,
    resolved_dir: DoctorPath,
    debug_log: DoctorPath,
}

#[derive(Debug, Serialize)]
struct DoctorNativeLibrary {
    load_ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ton_commit_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ton_commit_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

#[derive(Debug, Serialize)]
struct DoctorNativeLibraries {
    emulator: DoctorNativeLibrary,
    tolk: DoctorNativeLibrary,
}

#[derive(Debug, Serialize)]
struct DoctorEnvironmentVars {
    home: Option<String>,
    userprofile: Option<String>,
    ci: Option<String>,
    term: Option<String>,
    lang: Option<String>,
    shell: Option<String>,
    no_color: Option<String>,
}

#[derive(Debug, Serialize)]
struct DoctorEnvironment {
    current_dir: String,
    executable: String,
    vars: DoctorEnvironmentVars,
}

#[derive(Debug, Serialize)]
struct DoctorReport {
    versions: DoctorVersions,
    paths: DoctorPaths,
    config_overlays: DoctorConfigOverlays,
    manifest: DoctorManifest,
    stdlib: DoctorStdlib,
    native_libraries: DoctorNativeLibraries,
    logging: DoctorLogging,
    environment: DoctorEnvironment,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
enum DoctorApiMethod {
    Get,
    PostJson,
}

impl DoctorApiMethod {
    const fn as_str(&self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::PostJson => "POST",
        }
    }
}

#[derive(Debug, Clone)]
struct DoctorApiTarget {
    name: String,
    method: DoctorApiMethod,
    url: String,
    display_url: String,
    body: Option<serde_json::Value>,
    sequence_group: Option<String>,
    sequence_delay_after: Duration,
    retry_on_429_after: Option<Duration>,
}

#[derive(Debug, Clone, Deserialize)]
struct DoctorApiTargetOverride {
    name: String,
    method: DoctorApiMethod,
    url: String,
    #[serde(default)]
    display_url: Option<String>,
    #[serde(default)]
    body: Option<serde_json::Value>,
    #[serde(default)]
    sequence_group: Option<String>,
    #[serde(default)]
    sequence_delay_after_ms: Option<u64>,
    #[serde(default)]
    retry_on_429_after_ms: Option<u64>,
}

#[derive(Debug, Serialize)]
struct DoctorApiCheck {
    name: String,
    method: String,
    url: String,
    ok: bool,
    status_code: Option<u16>,
    duration_ms: u128,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

#[derive(Debug, Serialize)]
struct DoctorApiChecks {
    healthy: usize,
    total: usize,
    checks: Vec<DoctorApiCheck>,
}

#[derive(Debug, Deserialize)]
struct ManifestSummary {
    contracts: Option<BTreeMap<String, toml::Value>>,
    scripts: Option<BTreeMap<String, toml::Value>>,
    mappings: Option<BTreeMap<String, toml::Value>>,
}

struct OverlayInspection {
    report: DoctorOverlayFile,
    names: BTreeSet<String>,
    load_ok: bool,
}

fn is_writable(path: &Path) -> bool {
    match fs::metadata(path) {
        Ok(metadata) if metadata.is_dir() => tempfile::Builder::new()
            .prefix(".acton-doctor-write-check-")
            .tempfile_in(path)
            .is_ok(),
        Ok(_) => OpenOptions::new().write(true).open(path).is_ok(),
        Err(_) => {
            let mut candidate = path.parent();

            while let Some(current) = candidate {
                match fs::metadata(current) {
                    Ok(metadata) if metadata.is_dir() => {
                        return tempfile::Builder::new()
                            .prefix(".acton-doctor-write-check-")
                            .tempfile_in(current)
                            .is_ok();
                    }
                    Ok(_) | Err(_) => candidate = current.parent(),
                }
            }

            false
        }
    }
}

fn describe_path(path: &Path, resolution_source: Option<&str>) -> DoctorPath {
    describe_path_with_options(path, resolution_source, false)
}

fn describe_path_with_size(path: &Path, resolution_source: Option<&str>) -> DoctorPath {
    describe_path_with_options(path, resolution_source, true)
}

fn describe_path_with_options(
    path: &Path,
    resolution_source: Option<&str>,
    include_size: bool,
) -> DoctorPath {
    let size_bytes = if include_size {
        path_size_bytes(path)
    } else {
        None
    };

    DoctorPath {
        path: path.display().to_string(),
        exists: path.exists(),
        writable: is_writable(path),
        canonical_path: dunce::canonicalize(path)
            .ok()
            .map(|resolved| resolved.display().to_string()),
        size_human: size_bytes.map(format_size_human),
        size_bytes,
        resolution_source: resolution_source.map(ToOwned::to_owned),
    }
}

fn path_size_bytes(path: &Path) -> Option<u64> {
    let metadata = fs::symlink_metadata(path).ok()?;
    accumulate_path_size(path, &metadata).ok()
}

fn accumulate_path_size(path: &Path, metadata: &fs::Metadata) -> std::io::Result<u64> {
    let file_type = metadata.file_type();
    if file_type.is_symlink() {
        return Ok(0);
    }

    if file_type.is_file() {
        return Ok(metadata.len());
    }

    if !file_type.is_dir() {
        return Ok(0);
    }

    let mut total = 0_u64;
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let entry_path = entry.path();
        let entry_metadata = fs::symlink_metadata(&entry_path)?;
        total = total.saturating_add(accumulate_path_size(&entry_path, &entry_metadata)?);
    }

    Ok(total)
}

fn format_size_human(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];

    if bytes < 1024 {
        return format!("{bytes} B");
    }

    let mut value = bytes as f64;
    let mut unit_index = 0_usize;
    while value >= 1024.0 && unit_index + 1 < UNITS.len() {
        value /= 1024.0;
        unit_index += 1;
    }

    if value >= 10.0 {
        format!("{value:.0} {}", UNITS[unit_index])
    } else {
        format!("{value:.1} {}", UNITS[unit_index])
    }
}

fn inspect_manifest(path: &Path) -> DoctorManifest {
    if !path.exists() {
        return DoctorManifest {
            exists: false,
            parse_ok: false,
            error: None,
            contracts_count: None,
            scripts_count: None,
            mappings_count: None,
        };
    }

    match fs::read_to_string(path) {
        Ok(content) => {
            let summary = toml::from_str::<ManifestSummary>(&content).ok();

            match toml::from_str::<ActonConfig>(&content) {
                Ok(_) => DoctorManifest {
                    exists: true,
                    parse_ok: true,
                    error: None,
                    contracts_count: summary
                        .as_ref()
                        .and_then(|summary| summary.contracts.as_ref().map(BTreeMap::len)),
                    scripts_count: summary
                        .as_ref()
                        .and_then(|summary| summary.scripts.as_ref().map(BTreeMap::len)),
                    mappings_count: summary
                        .as_ref()
                        .and_then(|summary| summary.mappings.as_ref().map(BTreeMap::len)),
                },
                Err(err) => DoctorManifest {
                    exists: true,
                    parse_ok: false,
                    error: Some(err.to_string()),
                    contracts_count: summary
                        .as_ref()
                        .and_then(|summary| summary.contracts.as_ref().map(BTreeMap::len)),
                    scripts_count: summary
                        .as_ref()
                        .and_then(|summary| summary.scripts.as_ref().map(BTreeMap::len)),
                    mappings_count: summary
                        .as_ref()
                        .and_then(|summary| summary.mappings.as_ref().map(BTreeMap::len)),
                },
            }
        }
        Err(err) => DoctorManifest {
            exists: true,
            parse_ok: false,
            error: Some(err.to_string()),
            contracts_count: None,
            scripts_count: None,
            mappings_count: None,
        },
    }
}

fn inspect_overlay_file<T>(
    path: &Path,
    extract_names: impl Fn(&T) -> BTreeSet<String>,
) -> OverlayInspection
where
    T: DeserializeOwned,
{
    if !path.exists() {
        return OverlayInspection {
            report: DoctorOverlayFile {
                path: describe_path(path, None),
                parse_ok: None,
                error: None,
                entries_count: None,
            },
            names: BTreeSet::new(),
            load_ok: true,
        };
    }

    match fs::read_to_string(path) {
        Ok(content) => match toml::from_str::<T>(&content) {
            Ok(parsed) => {
                let names = extract_names(&parsed);
                OverlayInspection {
                    report: DoctorOverlayFile {
                        path: describe_path(path, None),
                        parse_ok: Some(true),
                        error: None,
                        entries_count: Some(names.len()),
                    },
                    names,
                    load_ok: true,
                }
            }
            Err(err) => OverlayInspection {
                report: DoctorOverlayFile {
                    path: describe_path(path, None),
                    parse_ok: Some(false),
                    error: Some(err.to_string()),
                    entries_count: None,
                },
                names: BTreeSet::new(),
                load_ok: false,
            },
        },
        Err(err) => OverlayInspection {
            report: DoctorOverlayFile {
                path: describe_path(path, None),
                parse_ok: Some(false),
                error: Some(err.to_string()),
                entries_count: None,
            },
            names: BTreeSet::new(),
            load_ok: false,
        },
    }
}

fn inspect_wallet_overlays(local_path: &Path, global_path: Option<&Path>) -> DoctorOverlayConfig {
    let local = inspect_overlay_file::<WalletsFile>(local_path, |file| {
        file.wallets
            .as_ref()
            .map(|wallets| wallets.wallets.keys().cloned().collect())
            .unwrap_or_default()
    });
    let global = global_path.map(|path| {
        inspect_overlay_file::<WalletsFile>(path, |file| {
            file.wallets
                .as_ref()
                .map(|wallets| wallets.wallets.keys().cloned().collect())
                .unwrap_or_default()
        })
    });
    let load_ok = local.load_ok && global.as_ref().is_none_or(|global| global.load_ok);
    let merged_entries_count = if load_ok {
        let mut merged = BTreeSet::new();
        if let Some(global) = &global {
            merged.extend(global.names.iter().cloned());
        }
        merged.extend(local.names.iter().cloned());
        Some(merged.len())
    } else {
        None
    };

    DoctorOverlayConfig {
        load_ok,
        merged_entries_count,
        local: local.report,
        global: global.map(|global| global.report),
    }
}

fn inspect_library_overlays(local_path: &Path, global_path: Option<&Path>) -> DoctorOverlayConfig {
    let local = inspect_overlay_file::<LibrariesFile>(local_path, |file| {
        file.libraries
            .as_ref()
            .map(|libraries| libraries.libraries.keys().cloned().collect())
            .unwrap_or_default()
    });
    let global = global_path.map(|path| {
        inspect_overlay_file::<LibrariesFile>(path, |file| {
            file.libraries
                .as_ref()
                .map(|libraries| libraries.libraries.keys().cloned().collect())
                .unwrap_or_default()
        })
    });
    let load_ok = local.load_ok && global.as_ref().is_none_or(|global| global.load_ok);
    let merged_entries_count = if load_ok {
        let mut merged = BTreeSet::new();
        if let Some(global) = &global {
            merged.extend(global.names.iter().cloned());
        }
        merged.extend(local.names.iter().cloned());
        Some(merged.len())
    } else {
        None
    };

    DoctorOverlayConfig {
        load_ok,
        merged_entries_count,
        local: local.report,
        global: global.map(|global| global.report),
    }
}

fn inspect_stdlib(acton_dir: &Path, stdlib_path: &Path) -> DoctorStdlib {
    let version = read_first_existing(&[&acton_dir.join(".version")]);
    let revision = read_first_existing(&[
        &stdlib_path.join(".revision"),
        &stdlib_path.join(".git_hash"),
        &stdlib_path.join("REVISION"),
        &stdlib_path.join("VERSION"),
    ]);
    let common_tolk = stdlib_path.join("common.tolk");
    let expected_version = stdlib::current_stdlib_version();
    let status = if !stdlib_path.exists() {
        "missing"
    } else if !common_tolk.exists() {
        "incomplete"
    } else if version.as_deref() == Some(expected_version.as_str()) {
        "healthy"
    } else if version.is_some() {
        "outdated"
    } else {
        "unknown-version"
    };

    DoctorStdlib {
        path: describe_path(stdlib_path, None),
        common_tolk: describe_path(&common_tolk, None),
        status: status.to_string(),
        version,
        expected_version,
        revision,
        source: "embedded-bundle".to_string(),
    }
}

fn inspect_native_libraries() -> DoctorNativeLibraries {
    let emulator = match ton_executor::native_emulator_version() {
        Ok(version) => DoctorNativeLibrary {
            load_ok: true,
            version: None,
            ton_commit_hash: Some(version.ton_commit_hash),
            ton_commit_date: Some(version.ton_commit_date),
            error: None,
        },
        Err(err) => DoctorNativeLibrary {
            load_ok: false,
            version: None,
            ton_commit_hash: None,
            ton_commit_date: None,
            error: Some(err.to_string()),
        },
    };

    let tolk = match tolkc::native_tolk_version() {
        Ok(version) => DoctorNativeLibrary {
            load_ok: true,
            version: Some(version.version),
            ton_commit_hash: Some(version.ton_commit_hash),
            ton_commit_date: Some(version.ton_commit_date),
            error: None,
        },
        Err(err) => DoctorNativeLibrary {
            load_ok: false,
            version: None,
            ton_commit_hash: None,
            ton_commit_date: None,
            error: Some(err.to_string()),
        },
    };

    DoctorNativeLibraries { emulator, tolk }
}

fn non_empty_env_string(var: &str) -> Option<String> {
    env::var(var).ok().filter(|value| !value.is_empty())
}

fn env_path(var: &str) -> Option<PathBuf> {
    let value = env::var_os(var)?;
    if value.is_empty() {
        return None;
    }
    Some(PathBuf::from(value))
}

fn resolve_acton_log_dir(project_root: &Path) -> (PathBuf, &'static str) {
    if let Some(path) = env_path("ACTON_LOG_DIR") {
        return (path, "ACTON_LOG_DIR");
    }

    #[cfg(target_os = "windows")]
    {
        if let Some(path) = env_path("USERPROFILE") {
            return (path.join(".acton").join("logs"), "USERPROFILE");
        }
        if let Some(path) = env_path("HOME") {
            return (path.join(".acton").join("logs"), "HOME");
        }
        return (paths::build_logs_dir(project_root), "project_root");
    }

    #[cfg(not(target_os = "windows"))]
    {
        if let Some(path) = env_path("HOME") {
            return (path.join(".acton").join("logs"), "HOME");
        }
        (paths::build_logs_dir(project_root), "project_root")
    }
}

fn read_runtime_path(label: &str, result: std::io::Result<PathBuf>) -> String {
    match result {
        Ok(path) => path.display().to_string(),
        Err(err) => format!("<unavailable: {label}: {err}>"),
    }
}

fn collect_doctor_report() -> Result<DoctorReport> {
    let resolved_paths = resolved_paths_diagnostics();
    let project_root = resolved_paths.project_root;
    let manifest_path = resolved_paths.manifest_path;
    let manifest_status = inspect_manifest(&manifest_path);
    let project_root_source = resolved_paths.project_root_source.as_str();
    let manifest_source = resolved_paths.manifest_path_source.as_str();

    let acton_dir = project_root.join(".acton");
    let cache_dir = paths::build_cache_dir(&project_root);
    let local_wallets = project_root.join("wallets.toml");
    let local_libraries = project_root.join("libraries.toml");
    let global_wallets = global_wallets_path();
    let global_libraries = global_libraries_path();

    let stdlib_path = acton_dir.join("tolk-stdlib");
    let stdlib = inspect_stdlib(&acton_dir, &stdlib_path);
    let native_libraries = inspect_native_libraries();
    let wallets_overlay = inspect_wallet_overlays(&local_wallets, global_wallets.as_deref());
    let libraries_overlay = inspect_library_overlays(&local_libraries, global_libraries.as_deref());
    let (resolved_log_dir, log_dir_source) = resolve_acton_log_dir(&project_root);
    let debug_log = resolved_log_dir.join("debug.log");

    let current_dir = read_runtime_path("current_dir", env::current_dir());
    let executable = read_runtime_path("current_exe", env::current_exe());

    Ok(DoctorReport {
        versions: DoctorVersions {
            acton: build_info::PACKAGE_VERSION.to_string(),
            release_channel: build_info::RELEASE_CHANNEL.to_string(),
            git_sha: build_info::GIT_HASH.to_string(),
            build_date: build_info::BUILD_DATE.to_string(),
            target_triple: build_info::TARGET_TRIPLE.to_string(),
            profile: build_info::BUILD_PROFILE.to_string(),
            os: env::consts::OS.to_string(),
            arch: env::consts::ARCH.to_string(),
        },
        paths: DoctorPaths {
            project_root: describe_path(&project_root, Some(project_root_source)),
            manifest_path: describe_path(&manifest_path, Some(manifest_source)),
            acton_dir: describe_path(&acton_dir, None),
            cache_dir: describe_path_with_size(&cache_dir, None),
            wallets: describe_path(&local_wallets, None),
            global_wallets: global_wallets
                .as_deref()
                .map(|path| describe_path(path, None)),
            libraries: describe_path(&local_libraries, None),
            global_libraries: global_libraries
                .as_deref()
                .map(|path| describe_path(path, None)),
        },
        config_overlays: DoctorConfigOverlays {
            wallets: wallets_overlay,
            libraries: libraries_overlay,
        },
        manifest: manifest_status,
        stdlib,
        native_libraries,
        logging: DoctorLogging {
            acton_log_dir_env: non_empty_env_string("ACTON_LOG_DIR"),
            resolved_dir: describe_path(&resolved_log_dir, Some(log_dir_source)),
            debug_log: describe_path(&debug_log, None),
        },
        environment: DoctorEnvironment {
            current_dir,
            executable,
            vars: DoctorEnvironmentVars {
                home: env::var("HOME").ok(),
                userprofile: env::var("USERPROFILE").ok(),
                ci: env::var("CI").ok(),
                term: env::var("TERM").ok(),
                lang: env::var("LANG").ok(),
                shell: env::var("SHELL").ok(),
                no_color: env::var("NO_COLOR").ok(),
            },
        },
    })
}

fn read_first_existing(paths: &[&Path]) -> Option<String> {
    for path in paths {
        if path.exists()
            && let Ok(content) = fs::read_to_string(path)
        {
            let value = content.trim();
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}

fn build_doctor_api_targets() -> Result<Vec<DoctorApiTarget>> {
    if let Ok(raw) = env::var(DOCTOR_API_TARGETS_JSON_ENV) {
        let overrides: Vec<DoctorApiTargetOverride> = serde_json::from_str(&raw)?;
        let targets = overrides
            .into_iter()
            .map(|item| DoctorApiTarget {
                display_url: item.display_url.unwrap_or_else(|| item.url.clone()),
                name: item.name,
                method: item.method,
                url: item.url,
                body: item.body,
                sequence_group: item.sequence_group,
                sequence_delay_after: Duration::from_millis(
                    item.sequence_delay_after_ms.unwrap_or_default(),
                ),
                retry_on_429_after: item.retry_on_429_after_ms.map(Duration::from_millis),
            })
            .collect();
        return Ok(targets);
    }

    let dton_api_key = env::var("DTON_API_KEY").unwrap_or_else(|_| DEFAULT_DTON_API_KEY.to_owned());

    Ok(build_default_doctor_api_targets(&dton_api_key))
}

fn build_default_doctor_api_targets(dton_api_key: &str) -> Vec<DoctorApiTarget> {
    let graphql_probe = Some(serde_json::json!({
        "query": "query { __typename }",
        "variables": {}
    }));

    vec![
        DoctorApiTarget {
            name: "toncenter_v2_mainnet".to_string(),
            method: DoctorApiMethod::Get,
            url: "https://toncenter.com/api/v2/getMasterchainInfo".to_string(),
            display_url: "https://toncenter.com/api/v2/getMasterchainInfo".to_string(),
            body: None,
            sequence_group: Some("toncenter".to_string()),
            sequence_delay_after: DOCTOR_TONCENTER_REQUEST_STAGGER,
            retry_on_429_after: Some(DOCTOR_TONCENTER_REQUEST_STAGGER),
        },
        DoctorApiTarget {
            name: "toncenter_v2_testnet".to_string(),
            method: DoctorApiMethod::Get,
            url: "https://testnet.toncenter.com/api/v2/getMasterchainInfo".to_string(),
            display_url: "https://testnet.toncenter.com/api/v2/getMasterchainInfo".to_string(),
            body: None,
            sequence_group: Some("toncenter".to_string()),
            sequence_delay_after: DOCTOR_TONCENTER_REQUEST_STAGGER,
            retry_on_429_after: Some(DOCTOR_TONCENTER_REQUEST_STAGGER),
        },
        DoctorApiTarget {
            name: "toncenter_v3_mainnet".to_string(),
            method: DoctorApiMethod::Get,
            url: "https://toncenter.com/api/v3/masterchainInfo".to_string(),
            display_url: "https://toncenter.com/api/v3/masterchainInfo".to_string(),
            body: None,
            sequence_group: Some("toncenter".to_string()),
            sequence_delay_after: DOCTOR_TONCENTER_REQUEST_STAGGER,
            retry_on_429_after: Some(DOCTOR_TONCENTER_REQUEST_STAGGER),
        },
        DoctorApiTarget {
            name: "toncenter_v3_testnet".to_string(),
            method: DoctorApiMethod::Get,
            url: "https://testnet.toncenter.com/api/v3/masterchainInfo".to_string(),
            display_url: "https://testnet.toncenter.com/api/v3/masterchainInfo".to_string(),
            body: None,
            sequence_group: Some("toncenter".to_string()),
            sequence_delay_after: DOCTOR_TONCENTER_REQUEST_STAGGER,
            retry_on_429_after: Some(DOCTOR_TONCENTER_REQUEST_STAGGER),
        },
        DoctorApiTarget {
            name: "tonhub_v4_mainnet".to_string(),
            method: DoctorApiMethod::Get,
            url: "https://mainnet-v4.tonhubapi.com/block/latest".to_string(),
            display_url: "https://mainnet-v4.tonhubapi.com/block/latest".to_string(),
            body: None,
            sequence_group: None,
            sequence_delay_after: Duration::ZERO,
            retry_on_429_after: None,
        },
        DoctorApiTarget {
            name: "tonhub_v4_testnet".to_string(),
            method: DoctorApiMethod::Get,
            url: "https://testnet-v4.tonhubapi.com/block/latest".to_string(),
            display_url: "https://testnet-v4.tonhubapi.com/block/latest".to_string(),
            body: None,
            sequence_group: None,
            sequence_delay_after: Duration::ZERO,
            retry_on_429_after: None,
        },
        DoctorApiTarget {
            name: "dton_graphql_mainnet".to_string(),
            method: DoctorApiMethod::PostJson,
            url: format!("https://dton.io/{dton_api_key}/graphql"),
            display_url: "https://dton.io/[DTON_API_KEY]/graphql".to_string(),
            body: graphql_probe,
            sequence_group: None,
            sequence_delay_after: Duration::ZERO,
            retry_on_429_after: None,
        },
        DoctorApiTarget {
            name: "verifier_backends_config".to_string(),
            method: DoctorApiMethod::Get,
            url: "https://raw.githubusercontent.com/ton-community/contract-verifier-config/main/config.json".to_string(),
            display_url: "https://raw.githubusercontent.com/ton-community/contract-verifier-config/main/config.json".to_string(),
            body: None,
            sequence_group: None,
            sequence_delay_after: Duration::ZERO,
            retry_on_429_after: None,
        },
    ]
}

fn send_doctor_api_request(
    client: &reqwest::blocking::Client,
    target: &DoctorApiTarget,
) -> Result<reqwest::blocking::Response, reqwest::Error> {
    match (&target.method, &target.body) {
        (DoctorApiMethod::Get, _) => client.get(&target.url),
        (DoctorApiMethod::PostJson, Some(body)) => client.post(&target.url).json(body),
        (DoctorApiMethod::PostJson, None) => client.post(&target.url),
    }
    .header("User-Agent", "acton-doctor")
    .send()
}

fn build_doctor_api_client() -> Result<reqwest::blocking::Client, reqwest::Error> {
    // `doctor` is best-effort diagnostics: prefer direct requests over
    // reqwest system-proxy autodiscovery, which can panic in restricted
    // macOS environments instead of returning a recoverable transport error.
    reqwest::blocking::Client::builder()
        .no_proxy()
        .connect_timeout(DOCTOR_API_CONNECT_TIMEOUT)
        .timeout(DOCTOR_API_REQUEST_TIMEOUT)
        .build()
}

fn doctor_api_panic_message(payload: &(dyn Any + Send)) -> String {
    if let Some(message) = payload.downcast_ref::<&str>() {
        (*message).to_string()
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else {
        "unknown panic payload".to_string()
    }
}

fn doctor_api_transport_error(error: impl std::fmt::Display) -> String {
    format!("request failed before receiving an HTTP response from this environment: {error}")
}

fn panic_doctor_api_check(
    target: DoctorApiTarget,
    started: Instant,
    payload: Box<dyn Any + Send>,
) -> DoctorApiCheck {
    DoctorApiCheck {
        name: target.name,
        method: target.method.as_str().to_string(),
        url: target.display_url,
        ok: false,
        status_code: None,
        duration_ms: started.elapsed().as_millis(),
        error: Some(format!(
            "API check panicked before receiving an HTTP response from this environment: {}",
            doctor_api_panic_message(payload.as_ref())
        )),
    }
}

fn run_doctor_api_check_with<F>(target: DoctorApiTarget, execute: F) -> DoctorApiCheck
where
    F: FnOnce(&DoctorApiTarget, Instant) -> DoctorApiCheck,
{
    let started = Instant::now();
    match std::panic::catch_unwind(AssertUnwindSafe(|| execute(&target, started))) {
        Ok(check) => check,
        Err(payload) => panic_doctor_api_check(target, started, payload),
    }
}

fn run_doctor_api_check_inner(target: &DoctorApiTarget, started: Instant) -> DoctorApiCheck {
    let client = build_doctor_api_client();

    let (ok, status_code, error) = match client {
        Ok(client) => match send_doctor_api_request(&client, target) {
            Ok(response) => {
                let mut status = response.status().as_u16();
                if status == 429
                    && let Some(retry_delay) = target.retry_on_429_after
                {
                    std::thread::sleep(retry_delay);
                    match send_doctor_api_request(&client, target) {
                        Ok(retry_response) => {
                            status = retry_response.status().as_u16();
                            (
                                status == 200,
                                Some(status),
                                (status != 200).then(|| format!("HTTP {status}")),
                            )
                        }
                        Err(err) => (false, None, Some(doctor_api_transport_error(err))),
                    }
                } else {
                    (
                        status == 200,
                        Some(status),
                        (status != 200).then(|| format!("HTTP {status}")),
                    )
                }
            }
            Err(err) => (false, None, Some(doctor_api_transport_error(err))),
        },
        Err(err) => (false, None, Some(doctor_api_transport_error(err))),
    };

    let duration_ms = started.elapsed().as_millis();

    DoctorApiCheck {
        name: target.name.clone(),
        method: target.method.as_str().to_string(),
        url: target.display_url.clone(),
        ok,
        status_code,
        duration_ms,
        error,
    }
}

fn run_doctor_api_check(target: DoctorApiTarget) -> DoctorApiCheck {
    run_doctor_api_check_with(target, run_doctor_api_check_inner)
}

fn run_doctor_api_group(group: Vec<(usize, DoctorApiTarget)>) -> Vec<(usize, DoctorApiCheck)> {
    let group_len = group.len();
    let mut results = Vec::with_capacity(group_len);

    for (position, (index, target)) in group.into_iter().enumerate() {
        let sequence_delay_after = target.sequence_delay_after;
        results.push((index, run_doctor_api_check(target)));

        if position + 1 < group_len && !sequence_delay_after.is_zero() {
            std::thread::sleep(sequence_delay_after);
        }
    }

    results
}

fn collect_doctor_api_checks() -> DoctorApiChecks {
    let targets = match build_doctor_api_targets() {
        Ok(targets) => targets,
        Err(err) => {
            return DoctorApiChecks {
                healthy: 0,
                total: 1,
                checks: vec![DoctorApiCheck {
                    name: "api_targets".to_string(),
                    method: "<config>".to_string(),
                    url: format!("env:{DOCTOR_API_TARGETS_JSON_ENV}"),
                    ok: false,
                    status_code: None,
                    duration_ms: 0,
                    error: Some(err.to_string()),
                }],
            };
        }
    };

    let total = targets.len();
    let mut groups = Vec::<Vec<(usize, DoctorApiTarget)>>::new();
    let mut grouped_indexes = BTreeMap::<String, usize>::new();

    for (index, target) in targets.into_iter().enumerate() {
        if let Some(group_name) = target.sequence_group.clone() {
            let group_index = if let Some(index) = grouped_indexes.get(&group_name) {
                *index
            } else {
                let next_index = groups.len();
                grouped_indexes.insert(group_name, next_index);
                groups.push(Vec::new());
                next_index
            };
            groups[group_index].push((index, target));
        } else {
            groups.push(vec![(index, target)]);
        }
    }

    let mut handles = Vec::with_capacity(groups.len());
    for group in groups {
        handles.push(std::thread::spawn(move || run_doctor_api_group(group)));
    }

    let mut indexed = Vec::with_capacity(total);
    for handle in handles {
        match handle.join() {
            Ok(results) => indexed.extend(results),
            Err(_) => indexed.push((
                usize::MAX,
                DoctorApiCheck {
                    name: "api_check".to_string(),
                    method: "<thread>".to_string(),
                    url: "<unknown>".to_string(),
                    ok: false,
                    status_code: None,
                    duration_ms: 0,
                    error: Some("API check thread panicked".to_string()),
                },
            )),
        }
    }

    indexed.sort_by_key(|(index, _)| *index);
    let checks: Vec<_> = indexed.into_iter().map(|(_, check)| check).collect();
    let healthy = checks.iter().filter(|check| check.ok).count();

    DoctorApiChecks {
        healthy,
        total: checks.len(),
        checks,
    }
}

fn print_section(title: &str) {
    println!("{}", title.bold().cyan());
}

fn print_kv(label: &str, value: impl AsRef<str>) {
    let key = format!("{label}:");
    println!("{} {}", format!("{key:<17}").bold(), value.as_ref());
}

fn doctor_api_environment_note(report: &DoctorApiChecks) -> Option<String> {
    let transport_failures = report
        .checks
        .iter()
        .filter(|check| !check.ok && check.status_code.is_none())
        .count();

    if transport_failures == 0 {
        None
    } else if transport_failures == report.total && report.total > 0 {
        Some(
            "API health could not be verified from this environment; all outbound checks failed before any HTTP response. This does not prove the APIs are down.".to_string(),
        )
    } else {
        Some(format!(
            "{transport_failures} endpoint(s) could not be verified from this environment because the request failed before any HTTP response."
        ))
    }
}

fn print_path(label: &str, value: &DoctorPath) {
    let state = if value.exists {
        "exists".green().to_string()
    } else {
        "missing".yellow().to_string()
    };
    let writable = if value.writable {
        "writable".green().to_string()
    } else {
        "readonly".yellow().to_string()
    };

    let mut tags = vec![state, writable];
    if let Some(source) = &value.resolution_source {
        tags.push(format!("source={}", source.cyan()));
    }
    print_kv(label, format!("{} [{}]", value.path, tags.join(", ")));

    if let Some(canonical_path) = &value.canonical_path
        && canonical_path != &value.path
    {
        print_kv(&format!("{label}.canonical"), canonical_path);
    }

    if let Some(size_human) = &value.size_human {
        let size_value = match value.size_bytes {
            Some(bytes) => format!("{size_human} ({bytes} bytes)"),
            None => size_human.clone(),
        };
        print_kv(&format!("{label}.size"), size_value);
    }
}

fn print_overlay_file(label: &str, value: &DoctorOverlayFile) {
    print_path(&format!("{label}.path"), &value.path);
    print_kv(
        &format!("{label}.parse_ok"),
        value
            .parse_ok
            .map_or_else(|| "<n/a>".to_string(), |value| value.to_string()),
    );
    print_kv(
        &format!("{label}.entries"),
        value
            .entries_count
            .map_or_else(|| "<n/a>".to_string(), |value| value.to_string()),
    );
    match &value.error {
        Some(error) => print_kv(&format!("{label}.error"), error),
        None => print_kv(&format!("{label}.error"), "<none>"),
    }
}

fn print_report(report: &DoctorReport) {
    println!("{}", "Acton Doctor".bold().bright_cyan());
    println!();

    print_section("Versions");
    print_kv("acton", &report.versions.acton);
    print_kv("channel", &report.versions.release_channel);
    print_kv("git_sha", &report.versions.git_sha);
    print_kv("build_date", &report.versions.build_date);
    print_kv("target", &report.versions.target_triple);
    print_kv("profile", &report.versions.profile);
    print_kv("os", &report.versions.os);
    print_kv("arch", &report.versions.arch);
    println!();

    print_section("Paths");
    print_path("project_root", &report.paths.project_root);
    print_path("manifest_path", &report.paths.manifest_path);
    print_path("acton_dir", &report.paths.acton_dir);
    print_path("cache_dir", &report.paths.cache_dir);
    print_path("wallets", &report.paths.wallets);
    match &report.paths.global_wallets {
        Some(path) => print_path("global_wallets", path),
        None => print_kv(
            "global_wallets",
            "unavailable (HOME/USERPROFILE is not set)",
        ),
    }
    print_path("libraries", &report.paths.libraries);
    match &report.paths.global_libraries {
        Some(path) => print_path("global_libraries", path),
        None => print_kv(
            "global_libraries",
            "unavailable (HOME/USERPROFILE is not set)",
        ),
    }
    println!();

    print_section("Acton.toml");
    print_kv("exists", report.manifest.exists.to_string());
    print_kv("parse_ok", report.manifest.parse_ok.to_string());
    match &report.manifest.error {
        Some(error) => print_kv("error", error),
        None => print_kv("error", "<none>"),
    }
    print_kv(
        "contracts",
        report
            .manifest
            .contracts_count
            .map_or_else(|| "<n/a>".to_string(), |v| v.to_string()),
    );
    print_kv(
        "scripts",
        report
            .manifest
            .scripts_count
            .map_or_else(|| "<n/a>".to_string(), |v| v.to_string()),
    );
    print_kv(
        "mappings",
        report
            .manifest
            .mappings_count
            .map_or_else(|| "<n/a>".to_string(), |v| v.to_string()),
    );
    println!();

    print_section("Wallet Overlays");
    print_kv(
        "load_ok",
        report.config_overlays.wallets.load_ok.to_string(),
    );
    print_kv(
        "merged_entries",
        report
            .config_overlays
            .wallets
            .merged_entries_count
            .map_or_else(|| "<n/a>".to_string(), |value| value.to_string()),
    );
    print_overlay_file("local", &report.config_overlays.wallets.local);
    match &report.config_overlays.wallets.global {
        Some(global) => print_overlay_file("global", global),
        None => print_kv("global", "unavailable (HOME/USERPROFILE is not set)"),
    }
    println!();

    print_section("Library Overlays");
    print_kv(
        "load_ok",
        report.config_overlays.libraries.load_ok.to_string(),
    );
    print_kv(
        "merged_entries",
        report
            .config_overlays
            .libraries
            .merged_entries_count
            .map_or_else(|| "<n/a>".to_string(), |value| value.to_string()),
    );
    print_overlay_file("local", &report.config_overlays.libraries.local);
    match &report.config_overlays.libraries.global {
        Some(global) => print_overlay_file("global", global),
        None => print_kv("global", "unavailable (HOME/USERPROFILE is not set)"),
    }
    println!();

    print_section("Stdlib");
    print_path("path", &report.stdlib.path);
    print_path("common_tolk", &report.stdlib.common_tolk);
    print_kv("status", &report.stdlib.status);
    print_kv(
        "version",
        report.stdlib.version.as_deref().unwrap_or("<unknown>"),
    );
    print_kv("expected_version", &report.stdlib.expected_version);
    print_kv(
        "revision",
        report.stdlib.revision.as_deref().unwrap_or("<unknown>"),
    );
    print_kv("source", &report.stdlib.source);
    println!();

    print_section("Native Libraries");
    print_kv(
        "emulator.load_ok",
        report.native_libraries.emulator.load_ok.to_string(),
    );
    print_kv(
        "emulator.version",
        report
            .native_libraries
            .emulator
            .version
            .as_deref()
            .unwrap_or("<n/a>"),
    );
    print_kv(
        "emulator.ton_commit_hash",
        report
            .native_libraries
            .emulator
            .ton_commit_hash
            .as_deref()
            .unwrap_or("<unknown>"),
    );
    print_kv(
        "emulator.ton_commit_date",
        report
            .native_libraries
            .emulator
            .ton_commit_date
            .as_deref()
            .unwrap_or("<unknown>"),
    );
    print_kv(
        "emulator.error",
        report
            .native_libraries
            .emulator
            .error
            .as_deref()
            .unwrap_or("<none>"),
    );
    print_kv(
        "tolk.load_ok",
        report.native_libraries.tolk.load_ok.to_string(),
    );
    print_kv(
        "tolk.version",
        report
            .native_libraries
            .tolk
            .version
            .as_deref()
            .unwrap_or("<unknown>"),
    );
    print_kv(
        "tolk.ton_commit_hash",
        report
            .native_libraries
            .tolk
            .ton_commit_hash
            .as_deref()
            .unwrap_or("<unknown>"),
    );
    print_kv(
        "tolk.ton_commit_date",
        report
            .native_libraries
            .tolk
            .ton_commit_date
            .as_deref()
            .unwrap_or("<unknown>"),
    );
    print_kv(
        "tolk.error",
        report
            .native_libraries
            .tolk
            .error
            .as_deref()
            .unwrap_or("<none>"),
    );
    println!();

    print_section("Logging");
    print_kv(
        "ACTON_LOG_DIR",
        report
            .logging
            .acton_log_dir_env
            .as_deref()
            .unwrap_or("<unset>"),
    );
    print_path("resolved_dir", &report.logging.resolved_dir);
    print_path("debug_log", &report.logging.debug_log);
    println!();

    print_section("Environment");
    print_kv("current_dir", &report.environment.current_dir);
    print_kv("executable", &report.environment.executable);
    print_kv(
        "HOME",
        report.environment.vars.home.as_deref().unwrap_or("<unset>"),
    );
    print_kv(
        "USERPROFILE",
        report
            .environment
            .vars
            .userprofile
            .as_deref()
            .unwrap_or("<unset>"),
    );
    print_kv(
        "CI",
        report.environment.vars.ci.as_deref().unwrap_or("<unset>"),
    );
    print_kv(
        "TERM",
        report.environment.vars.term.as_deref().unwrap_or("<unset>"),
    );
    print_kv(
        "LANG",
        report.environment.vars.lang.as_deref().unwrap_or("<unset>"),
    );
    print_kv(
        "SHELL",
        report
            .environment
            .vars
            .shell
            .as_deref()
            .unwrap_or("<unset>"),
    );
    print_kv(
        "NO_COLOR",
        report
            .environment
            .vars
            .no_color
            .as_deref()
            .unwrap_or("<unset>"),
    );
}

fn print_api_checks(report: &DoctorApiChecks) {
    print_kv("healthy", format!("{}/{}", report.healthy, report.total));
    if let Some(note) = doctor_api_environment_note(report) {
        print_kv("note", note);
    }

    for check in &report.checks {
        let status = if check.ok {
            format!("{} [200, {} ms]", "ok".green(), check.duration_ms)
        } else if let Some(code) = check.status_code {
            format!(
                "{} [{code}, {} ms]",
                "failed".bright_red(),
                check.duration_ms
            )
        } else {
            format!("{} [{} ms]", "unverified".yellow(), check.duration_ms)
        };

        print_kv(
            &check.name,
            format!("{status} {} {}", check.method.dimmed(), check.url),
        );

        if let Some(error) = &check.error {
            print_kv(&format!("{}.error", check.name), error);
        }
    }
}

pub fn doctor_cmd() -> Result<()> {
    let report = collect_doctor_report()?;
    print_report(&report);
    println!();
    print_section("API Reachability");
    let _ = std::io::stdout().flush();
    let api_checks = collect_doctor_api_checks();
    print_api_checks(&api_checks);
    Ok(())
}
