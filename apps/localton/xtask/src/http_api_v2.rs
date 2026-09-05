use std::{fs, path::PathBuf};

use anyhow::{Context, Result, bail, ensure};
use tokio::process::Command;
use tracing::info;

use crate::build::{
    BuildArgs, Paths, absolute_path, checkout_commit, copy_dir_recursive, copy_executable_atomic,
    head_matches, init_checkout, prepend_path, run,
};

const UPSTREAM_REPOSITORY: &str = "https://github.com/toncenter/ton-http-api-cpp.git";
const UPSTREAM_BINARY: &str = "ton-http-api-cpp";
const UPSTREAM_BUILD_PATH: &str = "ton-http-api/ton-http-api-cpp";
const COMMIT: &str = "ab081891316b3513fb86d3815e33d141fdca2c6d";
const TON_COMMIT: &str = "bbc3bc6d52abbe3a7f852b22050708166fdaafbc";
const BUILD_SCHEMA: &str = "4";
const BROKEN_CACHE_CONSTRUCTION: &str = "cache_ = std::make_shared<Cache>(cache_size, way_size);";
const FIXED_CACHE_CONSTRUCTION: &str = "cache_ = std::make_shared<Cache>(cache_ways, way_size);";

pub async fn build(args: BuildArgs) -> Result<()> {
    let state_dir = absolute_path(&args.state_dir)?;
    let paths = Paths::new(
        &state_dir,
        args.install_dir.as_deref(),
        "ton-http-api-v2",
        "build",
    )?;
    let (repository, commit) = args.source(UPSTREAM_REPOSITORY, COMMIT, "TON_HTTP_API")?;
    fs::create_dir_all(&paths.root)
        .with_context(|| format!("failed to create {}", paths.root.display()))?;

    checkout_source(&paths, &repository, &commit).await?;
    install_suppression_mappings_workaround(&paths)?;
    install_cache_ways_workaround(&paths)?;
    let expected_stamp = build_stamp(&repository, &commit);
    let build_stamp = paths.build.join(".localton-build-version");
    let reusable = paths.build.join(UPSTREAM_BUILD_PATH).is_file()
        && fs::read_to_string(&build_stamp).is_ok_and(|stamp| stamp == expected_stamp);
    if reusable {
        info!("reusing the existing pinned TON HTTP API V2 build");
    } else {
        prepare_build_directory(&paths, &expected_stamp)?;
        configure(&paths).await?;
        compile(&paths, usize::from(args.jobs)).await?;
        fs::write(&build_stamp, &expected_stamp)
            .with_context(|| format!("failed to write {}", build_stamp.display()))?;
    }
    install(&paths)?;

    println!("TON HTTP API V2 source: {commit}");
    println!(
        "binary: {}",
        paths.install.join("bin/ton-http-api-v2").display()
    );
    println!(
        "config: {}",
        paths.install.join("config/static_config.yaml").display()
    );
    if args.install_dir.is_none() {
        println!(
            "run: localton bootstrap --state-dir {} --ton-http-api",
            state_dir.display()
        );
    }
    Ok(())
}

async fn checkout_source(paths: &Paths, repository: &str, commit: &str) -> Result<()> {
    init_checkout(&paths.source, repository).await?;
    if !head_matches(&paths.source, commit).await? {
        checkout_commit(&paths.source, commit).await?;
    }

    let mut sync = Command::new("git");
    sync.current_dir(&paths.source).args(["submodule", "sync"]);
    run("synchronize TON HTTP API V2 submodule remotes", &mut sync).await?;

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
    if !head_matches(&ton_source, TON_COMMIT).await? {
        checkout_commit(&ton_source, TON_COMMIT).await?;
    }

    let mut submodules = Command::new("git");
    submodules
        .args(["submodule", "update", "--init", "--depth", "1"])
        .current_dir(&ton_source);
    run("update matching TON source submodules", &mut submodules).await?;
    Ok(())
}

fn build_stamp(repository: &str, commit: &str) -> String {
    format!("{repository}\n{commit}\n{TON_COMMIT}\n{BUILD_SCHEMA}\n")
}

fn prepare_build_directory(paths: &Paths, expected_stamp: &str) -> Result<()> {
    let configure_stamp = paths.root.join(".localton-configure-version");
    let current_stamp = fs::read_to_string(&configure_stamp).ok();
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
    fs::write(&configure_stamp, expected_stamp)
        .with_context(|| format!("failed to write {}", configure_stamp.display()))?;
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
    fs::create_dir_all(paths.root.join("ccache"))?;
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
            // Avoid CPU-specific instructions from upstream's -march=native default.
            "-DTON_ARCH=",
            "-DPORTABLE=ON",
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
    command.env("CCACHE_DIR", paths.root.join("ccache"));

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
        prepend_path(command, "CPATH", &icu_root.join("include"))?;
        prepend_path(command, "LIBRARY_PATH", &icu_root.join("lib"))?;
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
    fs::create_dir_all(paths.install.join("bin"))?;
    fs::create_dir_all(paths.install.join("config"))?;
    copy_executable_atomic(
        &built_executable,
        &paths.install.join("bin/ton-http-api-v2"),
    )?;
    fs::copy(
        paths.source.join("config/static_config.yaml"),
        paths.install.join("config/static_config.yaml"),
    )
    .with_context(|| {
        format!(
            "failed to install TON HTTP API V2 config to {}",
            paths.install.join("config/static_config.yaml").display()
        )
    })?;
    copy_dir_recursive(
        &paths.source.join("ton-http-api/static"),
        &paths.install.join("static"),
    )?;
    Ok(())
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

    #[test]
    fn custom_installation_copies_runtime_files_and_keeps_the_build() {
        let temp = tempfile::tempdir().unwrap();
        let destination = temp.path().join("custom install");
        let paths =
            Paths::new(temp.path(), Some(&destination), "ton-http-api-v2", "build").unwrap();
        let binary = paths.build.join(UPSTREAM_BUILD_PATH);
        fs::create_dir_all(binary.parent().unwrap()).unwrap();
        fs::write(&binary, "compiled binary").unwrap();
        for (source, contents) in [
            ("config/static_config.yaml", "config"),
            ("ton-http-api/static/nested/openapi.json", "{}"),
        ] {
            let source = paths.source.join(source);
            fs::create_dir_all(source.parent().unwrap()).unwrap();
            fs::write(source, contents).unwrap();
        }

        install(&paths).unwrap();

        for (installed, contents) in [
            ("bin/ton-http-api-v2", "compiled binary"),
            ("config/static_config.yaml", "config"),
            ("static/nested/openapi.json", "{}"),
        ] {
            assert_eq!(
                fs::read_to_string(destination.join(installed)).unwrap(),
                contents
            );
        }
        assert!(binary.is_file());
        assert!(!paths.root.join("install").exists());
    }

    #[test]
    fn source_overrides_invalidate_the_build_cache() {
        let temp = tempfile::tempdir().unwrap();
        let paths = Paths::new(temp.path(), None, "ton-http-api-v2", "build").unwrap();
        let stamp = build_stamp(UPSTREAM_REPOSITORY, COMMIT);
        prepare_build_directory(&paths, &stamp).unwrap();
        let cached = paths.build.join("cached-object");
        fs::write(&cached, "compiled").unwrap();

        prepare_build_directory(&paths, &stamp).unwrap();
        assert!(cached.is_file());

        prepare_build_directory(&paths, &build_stamp("fork", COMMIT)).unwrap();
        assert!(!cached.exists());
        fs::write(&cached, "compiled fork").unwrap();
        prepare_build_directory(&paths, &build_stamp("fork", &"1".repeat(40))).unwrap();
        assert!(!cached.exists());
    }
}
