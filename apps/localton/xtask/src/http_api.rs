use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Stdio,
};

use anyhow::{Context, Result, bail, ensure};
use tokio::process::Command;
use tracing::info;

const UPSTREAM_REPOSITORY: &str = "https://github.com/toncenter/ton-http-api-cpp.git";
const UPSTREAM_BINARY: &str = "ton-http-api-cpp";
const UPSTREAM_BUILD_PATH: &str = "ton-http-api/ton-http-api-cpp";
const TAG: &str = "v2.1.13";
const COMMIT: &str = "ab081891316b3513fb86d3815e33d141fdca2c6d";
const TON_COMMIT: &str = "bbc3bc6d52abbe3a7f852b22050708166fdaafbc";
const BUILD_SCHEMA: &str = "3";
const BROKEN_CACHE_CONSTRUCTION: &str = "cache_ = std::make_shared<Cache>(cache_size, way_size);";
const FIXED_CACHE_CONSTRUCTION: &str = "cache_ = std::make_shared<Cache>(cache_ways, way_size);";

#[derive(Debug, Clone)]
struct Paths {
    root: PathBuf,
    source: PathBuf,
    build: PathBuf,
    ccache: PathBuf,
    build_stamp: PathBuf,
    configure_stamp: PathBuf,
    executable: PathBuf,
    static_config: PathBuf,
    static_content: PathBuf,
}

impl Paths {
    fn new(state_dir: &Path) -> Self {
        let root = state_dir.join("tools/ton-http-api-v2");
        let install = root.join("install");
        Self {
            source: root.join("source"),
            build: root.join("build"),
            ccache: root.join("ccache"),
            build_stamp: root.join("build/.localton-build-version"),
            configure_stamp: root.join(".localton-configure-version"),
            executable: install.join("bin/ton-http-api-v2"),
            static_config: install.join("config/static_config.yaml"),
            static_content: install.join("static"),
            root,
        }
    }
}

pub async fn build(state_dir: &Path, jobs: usize) -> Result<()> {
    let state_dir = absolute_path(state_dir)?;
    let paths = Paths::new(&state_dir);
    fs::create_dir_all(&paths.root)
        .with_context(|| format!("failed to create {}", paths.root.display()))?;

    checkout_source(&paths).await?;
    install_suppression_mappings_workaround(&paths)?;
    install_cache_ways_workaround(&paths)?;
    let expected_stamp = build_stamp();
    let reusable = paths.build.join(UPSTREAM_BUILD_PATH).is_file()
        && fs::read_to_string(&paths.build_stamp).is_ok_and(|stamp| stamp == expected_stamp);
    if reusable {
        info!("reusing the existing pinned TON HTTP API V2 build");
    } else {
        prepare_build_directory(&paths, &expected_stamp)?;
        configure(&paths).await?;
        compile(&paths, jobs).await?;
        fs::write(&paths.build_stamp, &expected_stamp)
            .with_context(|| format!("failed to write {}", paths.build_stamp.display()))?;
    }
    install(&paths)?;

    println!("TON HTTP API V2 {TAG}");
    println!("binary: {}", paths.executable.display());
    println!("config: {}", paths.static_config.display());
    println!(
        "run: localton run --state-dir {} --ton-http-api",
        state_dir.display()
    );
    Ok(())
}

async fn checkout_source(paths: &Paths) -> Result<()> {
    if !paths.source.exists() {
        let mut command = Command::new("git");
        command
            .args([
                "clone",
                "--branch",
                TAG,
                "--depth",
                "1",
                UPSTREAM_REPOSITORY,
            ])
            .arg(&paths.source);
        run("clone TON HTTP API V2 source", &mut command).await?;
    }

    ensure!(
        paths.source.join(".git").exists(),
        "{} exists but is not a TON HTTP API V2 source checkout",
        paths.source.display()
    );
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(&paths.source)
        .output()
        .await
        .context("failed to execute git rev-parse for TON HTTP API V2")?;
    ensure!(
        output.status.success(),
        "failed to inspect TON HTTP API V2 checkout: {}",
        String::from_utf8_lossy(&output.stderr).trim()
    );
    let commit = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if commit != COMMIT {
        info!(
            commit = COMMIT,
            "updating the pinned TON HTTP API V2 release"
        );
        let mut fetch = Command::new("git");
        fetch
            .args(["fetch", "--depth", "1", "origin", COMMIT])
            .current_dir(&paths.source);
        run("fetch pinned TON HTTP API V2 release", &mut fetch).await?;

        let mut checkout = Command::new("git");
        checkout
            .args(["checkout", "--detach", "--force", COMMIT])
            .current_dir(&paths.source);
        run("checkout pinned TON HTTP API V2 release", &mut checkout).await?;
    }

    let mut submodules = Command::new("git");
    submodules
        .args([
            "submodule",
            "update",
            "--init",
            "--depth",
            "1",
            "external/userver",
        ])
        .current_dir(&paths.source);
    run("update TON HTTP API V2 submodules", &mut submodules).await?;

    checkout_matching_ton_source(paths).await?;
    Ok(())
}

async fn checkout_matching_ton_source(paths: &Paths) -> Result<()> {
    let ton_source = paths.source.join("external/ton");
    if !ton_source.join(".git").exists() {
        let mut initialize = Command::new("git");
        initialize
            .args([
                "submodule",
                "update",
                "--init",
                "--depth",
                "1",
                "external/ton",
            ])
            .current_dir(&paths.source);
        run("initialize TON HTTP API V2 TON submodule", &mut initialize).await?;
    }
    ensure!(
        ton_source.join(".git").exists(),
        "TON HTTP API V2 TON submodule is missing: {}",
        ton_source.display()
    );
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(&ton_source)
        .output()
        .await
        .context("failed to inspect TON HTTP API V2 TON submodule")?;
    ensure!(
        output.status.success(),
        "failed to inspect TON submodule: {}",
        String::from_utf8_lossy(&output.stderr).trim()
    );
    let commit = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if commit != TON_COMMIT {
        info!(
            commit = TON_COMMIT,
            "updating TON HTTP API V2 tonlib to match validator-engine"
        );
        let mut fetch = Command::new("git");
        fetch
            .args(["fetch", "--depth", "1", "origin", TON_COMMIT])
            .current_dir(&ton_source);
        run("fetch matching TON source", &mut fetch).await?;

        let mut checkout = Command::new("git");
        checkout
            .args(["checkout", "--detach", "--force", TON_COMMIT])
            .current_dir(&ton_source);
        run("checkout matching TON source", &mut checkout).await?;
    }

    let mut submodules = Command::new("git");
    submodules
        .args(["submodule", "update", "--init", "--depth", "1"])
        .current_dir(&ton_source);
    run("update matching TON source submodules", &mut submodules).await?;
    Ok(())
}

fn build_stamp() -> String {
    format!("{COMMIT}\n{TON_COMMIT}\n{BUILD_SCHEMA}\n")
}

fn prepare_build_directory(paths: &Paths, expected_stamp: &str) -> Result<()> {
    let current_stamp = fs::read_to_string(&paths.configure_stamp).ok();
    if current_stamp.as_deref() == Some(expected_stamp) {
        return Ok(());
    }
    if paths.build.is_dir() {
        info!(
            build = %paths.build.display(),
            "recreating TON HTTP API V2 build directory for changed build inputs"
        );
        fs::remove_dir_all(&paths.build)
            .with_context(|| format!("failed to remove {}", paths.build.display()))?;
    }
    fs::create_dir_all(&paths.build)
        .with_context(|| format!("failed to create {}", paths.build.display()))?;
    fs::write(&paths.configure_stamp, expected_stamp)
        .with_context(|| format!("failed to write {}", paths.configure_stamp.display()))?;
    Ok(())
}

fn install_suppression_mappings_workaround(paths: &Paths) -> Result<()> {
    let expected = paths.source.join("suppression_mappings.txt");
    if expected.is_file() {
        return Ok(());
    }
    let actual = paths.source.join("external/ton/suppression_mappings.txt");
    ensure!(
        actual.is_file(),
        "TON warning suppression map is missing: {}",
        actual.display()
    );
    fs::copy(&actual, &expected).with_context(|| {
        format!(
            "failed to copy {} to {}",
            actual.display(),
            expected.display()
        )
    })?;
    Ok(())
}

fn install_cache_ways_workaround(paths: &Paths) -> Result<()> {
    let handler = paths
        .source
        .join("ton-http-api/src/handlers/TonlibRequestHandler.h");
    let source = fs::read_to_string(&handler)
        .with_context(|| format!("failed to read {}", handler.display()))?;
    let Some(patched) = patch_cache_ways_source(&source)? else {
        return Ok(());
    };
    fs::write(&handler, patched)
        .with_context(|| format!("failed to patch {}", handler.display()))?;
    Ok(())
}

fn patch_cache_ways_source(source: &str) -> Result<Option<String>> {
    let broken_count = source.matches(BROKEN_CACHE_CONSTRUCTION).count();
    let fixed_count = source.matches(FIXED_CACHE_CONSTRUCTION).count();
    match (broken_count, fixed_count) {
        (1, 0) => Ok(Some(source.replacen(
            BROKEN_CACHE_CONSTRUCTION,
            FIXED_CACHE_CONSTRUCTION,
            1,
        ))),
        (0, 1) => Ok(None),
        _ => bail!(
            "unexpected TON HTTP API V2 cache construction: found {broken_count} broken and {fixed_count} fixed occurrences"
        ),
    }
}

async fn configure(paths: &Paths) -> Result<()> {
    fs::create_dir_all(&paths.ccache)
        .with_context(|| format!("failed to create {}", paths.ccache.display()))?;
    let mut command = Command::new("cmake");
    command
        .arg("-S")
        .arg(&paths.source)
        .arg("-B")
        .arg(&paths.build)
        .args([
            "-G",
            "Ninja",
            "-DCMAKE_BUILD_TYPE=Release",
            "-DPY_TONLIB_MULTICLIENT=OFF",
            "-DTONLIB_MULTICLIENT_EXAMPLES=OFF",
            "-DBUILD_TON_PLAYGROUND=OFF",
            "-DTON_ONLY_TONLIB=ON",
            "-DUSE_QUIC=OFF",
            "-DUSERVER_BUILD_TESTS=OFF",
            "-DUSERVER_BUILD_SAMPLES=OFF",
            "-DUSERVER_FEATURE_UTEST=OFF",
            "-DUSERVER_FEATURE_TESTSUITE=OFF",
            "-DUSERVER_USE_STATIC_LIBS=ON",
            "-DCMAKE_INTERPROCEDURAL_OPTIMIZATION=OFF",
        ]);
    apply_native_build_environment(&mut command, paths)?;
    run("configure TON HTTP API V2", &mut command).await?;

    let generated_python = paths.build.join("venv-userver-chaotic/bin/python3");
    ensure!(
        generated_python.is_file(),
        "userver build Python is missing: {}",
        generated_python.display()
    );
    let mut python_command = Command::new("cmake");
    python_command
        .arg("-S")
        .arg(&paths.source)
        .arg("-B")
        .arg(&paths.build)
        .args([
            "-USECP256K1_LIBRARY",
            "-USECP256K1_INCLUDE_DIR",
            "-UZLIB_*",
            "-UBLST_LIB",
        ])
        .arg(format!(
            "-DPython3_EXECUTABLE={}",
            generated_python.display()
        ));
    apply_native_build_environment(&mut python_command, paths)?;
    run(
        "configure TON HTTP API V2 Python environment",
        &mut python_command,
    )
    .await
}

async fn compile(paths: &Paths, jobs: usize) -> Result<()> {
    let mut command = Command::new("cmake");
    command
        .arg("--build")
        .arg(&paths.build)
        .args(["--target", UPSTREAM_BINARY, "--parallel"])
        .arg(jobs.to_string());
    apply_native_build_environment(&mut command, paths)?;
    run("build TON HTTP API V2", &mut command).await
}

fn apply_native_build_environment(command: &mut Command, paths: &Paths) -> Result<()> {
    command.env("CCACHE_DIR", &paths.ccache);

    let icu_root = [
        "/opt/homebrew/opt/icu4c",
        "/opt/homebrew/opt/icu4c@78",
        "/usr/local/opt/icu4c",
        "/usr/local/opt/icu4c@78",
    ]
    .into_iter()
    .map(PathBuf::from)
    .find(|path| path.join("include").is_dir() && path.join("lib").is_dir());
    if let Some(icu_root) = icu_root {
        let mut include_paths = vec![icu_root.join("include")];
        if let Some(existing) = env::var_os("CPATH") {
            include_paths.extend(env::split_paths(&existing));
        }
        command.env(
            "CPATH",
            env::join_paths(include_paths).context("failed to compose native include path")?,
        );

        let mut library_paths = vec![icu_root.join("lib")];
        if let Some(existing) = env::var_os("LIBRARY_PATH") {
            library_paths.extend(env::split_paths(&existing));
        }
        command.env(
            "LIBRARY_PATH",
            env::join_paths(library_paths).context("failed to compose native library path")?,
        );
    }
    Ok(())
}

fn install(paths: &Paths) -> Result<()> {
    let built_executable = paths.build.join(UPSTREAM_BUILD_PATH);
    ensure!(
        built_executable.is_file(),
        "built TON HTTP API V2 executable is missing: {}",
        built_executable.display()
    );
    let bin_dir = paths
        .executable
        .parent()
        .context("TON HTTP API V2 install binary has no parent")?;
    let config_dir = paths
        .static_config
        .parent()
        .context("TON HTTP API V2 static config has no parent")?;
    fs::create_dir_all(bin_dir)
        .with_context(|| format!("failed to create {}", bin_dir.display()))?;
    fs::create_dir_all(config_dir)
        .with_context(|| format!("failed to create {}", config_dir.display()))?;
    copy_executable_atomic(&built_executable, &paths.executable)?;
    fs::copy(
        paths.source.join("config/static_config.yaml"),
        &paths.static_config,
    )
    .with_context(|| {
        format!(
            "failed to install TON HTTP API V2 config to {}",
            paths.static_config.display()
        )
    })?;
    copy_dir_recursive(
        &paths.source.join("ton-http-api/static"),
        &paths.static_content,
    )?;
    Ok(())
}

fn copy_executable_atomic(source: &Path, destination: &Path) -> Result<()> {
    let parent = destination
        .parent()
        .context("installed executable has no parent directory")?;
    let file_name = destination
        .file_name()
        .context("installed executable has no file name")?
        .to_string_lossy();
    let temporary = parent.join(format!(".{file_name}.{}.tmp", std::process::id()));

    let install_result = (|| {
        fs::copy(source, &temporary).with_context(|| {
            format!(
                "failed to copy {} to temporary executable {}",
                source.display(),
                temporary.display()
            )
        })?;
        fs::rename(&temporary, destination).with_context(|| {
            format!(
                "failed to atomically install {} to {}",
                source.display(),
                destination.display()
            )
        })?;
        Ok(())
    })();
    if install_result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    install_result
}

async fn run(description: &str, command: &mut Command) -> Result<()> {
    info!("{description}");
    let status = command
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .kill_on_drop(true)
        .status()
        .await
        .with_context(|| format!("failed to {description}"))?;
    if !status.success() {
        bail!("{description} failed with {status}");
    }
    Ok(())
}

fn copy_dir_recursive(source: &Path, destination: &Path) -> Result<()> {
    ensure!(
        source.is_dir(),
        "source directory is missing: {}",
        source.display()
    );
    fs::create_dir_all(destination)
        .with_context(|| format!("failed to create {}", destination.display()))?;
    for entry in
        fs::read_dir(source).with_context(|| format!("failed to read {}", source.display()))?
    {
        let entry = entry.with_context(|| format!("failed to read {}", source.display()))?;
        let target = destination.join(entry.file_name());
        if entry
            .file_type()
            .with_context(|| format!("failed to inspect {}", entry.path().display()))?
            .is_dir()
        {
            copy_dir_recursive(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), &target).with_context(|| {
                format!(
                    "failed to copy {} to {}",
                    entry.path().display(),
                    target.display()
                )
            })?;
        }
    }
    Ok(())
}

fn absolute_path(path: &Path) -> Result<PathBuf> {
    if path.is_absolute() {
        Ok(path.to_owned())
    } else {
        Ok(env::current_dir()?.join(path))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_workaround_uses_configured_way_count() {
        let source = format!("before\n  {BROKEN_CACHE_CONSTRUCTION}\nafter\n");
        let patched = patch_cache_ways_source(&source).unwrap().unwrap();

        assert!(!patched.contains(BROKEN_CACHE_CONSTRUCTION));
        assert!(patched.contains(FIXED_CACHE_CONSTRUCTION));
        assert!(patch_cache_ways_source(&patched).unwrap().is_none());
    }

    #[test]
    fn cache_workaround_rejects_unknown_source() {
        let error = patch_cache_ways_source("unrelated source").unwrap_err();

        assert!(
            error
                .to_string()
                .contains("unexpected TON HTTP API V2 cache construction")
        );
    }
}
