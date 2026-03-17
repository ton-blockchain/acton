use acton_config::color::OwoColorize;
use acton_config::config::{
    ActonConfig, LibrariesFile, WalletsFile, global_libraries_path, global_wallets_path,
    resolved_paths_diagnostics,
};
use anyhow::Result;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::OpenOptions;
use std::path::{Path, PathBuf};
use std::{env, fs};

#[derive(Debug, Serialize)]
struct DoctorPath {
    path: String,
    exists: bool,
    writable: bool,
    canonical_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    resolution_source: Option<String>,
}

#[derive(Debug, Serialize)]
struct DoctorVersions {
    acton: String,
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
    logging: DoctorLogging,
    environment: DoctorEnvironment,
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
    DoctorPath {
        path: path.display().to_string(),
        exists: path.exists(),
        writable: is_writable(path),
        canonical_path: dunce::canonicalize(path)
            .ok()
            .map(|resolved| resolved.display().to_string()),
        resolution_source: resolution_source.map(ToOwned::to_owned),
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
    let expected_version = env!("CARGO_PKG_VERSION").to_string();
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
        return (project_root.join(".acton").join("logs"), "project_root");
    }

    #[cfg(not(target_os = "windows"))]
    {
        if let Some(path) = env_path("HOME") {
            return (path.join(".acton").join("logs"), "HOME");
        }
        (project_root.join(".acton").join("logs"), "project_root")
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
    let cache_dir = project_root.join(".acton").join("cache");
    let local_wallets = project_root.join("wallets.toml");
    let local_libraries = project_root.join("libraries.toml");
    let global_wallets = global_wallets_path();
    let global_libraries = global_libraries_path();

    let stdlib_path = acton_dir.join("tolk-stdlib");
    let stdlib = inspect_stdlib(&acton_dir, &stdlib_path);
    let wallets_overlay = inspect_wallet_overlays(&local_wallets, global_wallets.as_deref());
    let libraries_overlay = inspect_library_overlays(&local_libraries, global_libraries.as_deref());
    let (resolved_log_dir, log_dir_source) = resolve_acton_log_dir(&project_root);
    let debug_log = resolved_log_dir.join("debug.log");

    let current_dir = read_runtime_path("current_dir", env::current_dir());
    let executable = read_runtime_path("current_exe", env::current_exe());

    Ok(DoctorReport {
        versions: DoctorVersions {
            acton: env!("CARGO_PKG_VERSION").to_string(),
            git_sha: env!("GIT_HASH").to_string(),
            build_date: env!("BUILD_DATE").to_string(),
            target_triple: env!("TARGET_TRIPLE").to_string(),
            profile: env!("BUILD_PROFILE").to_string(),
            os: env::consts::OS.to_string(),
            arch: env::consts::ARCH.to_string(),
        },
        paths: DoctorPaths {
            project_root: describe_path(&project_root, Some(project_root_source)),
            manifest_path: describe_path(&manifest_path, Some(manifest_source)),
            acton_dir: describe_path(&acton_dir, None),
            cache_dir: describe_path(&cache_dir, None),
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

fn print_section(title: &str) {
    println!("{}", title.bold().cyan());
}

fn print_kv(label: &str, value: impl AsRef<str>) {
    let key = format!("{label}:");
    println!("{} {}", format!("{key:<17}").bold(), value.as_ref());
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

pub fn doctor_cmd() -> Result<()> {
    let report = collect_doctor_report()?;
    print_report(&report);
    Ok(())
}
