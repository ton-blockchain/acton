use acton_config::color::OwoColorize;
use acton_config::config::{
    ActonConfig, global_libraries_path, global_wallets_path, resolved_paths_diagnostics,
};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;
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
struct DoctorStdlib {
    path: DoctorPath,
    version: Option<String>,
    revision: Option<String>,
    source: String,
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
    manifest: DoctorManifest,
    stdlib: DoctorStdlib,
    environment: DoctorEnvironment,
}

#[derive(Debug, Deserialize)]
struct ManifestSummary {
    contracts: Option<BTreeMap<String, toml::Value>>,
    scripts: Option<BTreeMap<String, toml::Value>>,
    mappings: Option<BTreeMap<String, toml::Value>>,
}

fn is_writable(path: &Path) -> bool {
    let metadata = if path.exists() {
        fs::metadata(path)
    } else {
        path.parent().map_or_else(
            || Err(std::io::Error::other("missing parent")),
            fs::metadata,
        )
    };

    metadata
        .map(|meta| !meta.permissions().readonly())
        .unwrap_or(false)
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

fn read_runtime_path(label: &str, result: std::io::Result<std::path::PathBuf>) -> String {
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

    let stdlib_path = acton_dir.join("tolk-stdlib");
    let stdlib_version = read_first_existing(&[&acton_dir.join(".version")]);
    let stdlib_revision = read_first_existing(&[
        &stdlib_path.join(".revision"),
        &stdlib_path.join(".git_hash"),
        &stdlib_path.join("REVISION"),
        &stdlib_path.join("VERSION"),
    ]);

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
            global_wallets: global_wallets_path()
                .as_deref()
                .map(|path| describe_path(path, None)),
            libraries: describe_path(&local_libraries, None),
            global_libraries: global_libraries_path()
                .as_deref()
                .map(|path| describe_path(path, None)),
        },
        manifest: manifest_status,
        stdlib: DoctorStdlib {
            path: describe_path(&stdlib_path, None),
            version: stdlib_version,
            revision: stdlib_revision,
            source: "embedded-bundle".to_string(),
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

    print_section("Stdlib");
    print_path("path", &report.stdlib.path);
    print_kv(
        "version",
        report.stdlib.version.as_deref().unwrap_or("<unknown>"),
    );
    print_kv(
        "revision",
        report.stdlib.revision.as_deref().unwrap_or("<unknown>"),
    );
    print_kv("source", &report.stdlib.source);
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

pub fn doctor_cmd(json: bool) -> Result<()> {
    let report = collect_doctor_report()?;
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        print_report(&report);
    }
    Ok(())
}
