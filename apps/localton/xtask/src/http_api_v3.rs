use std::{
    env, fs,
    path::{Path, PathBuf},
};

use anyhow::{Result, ensure};
use clap::{Args, ValueEnum};
use tokio::process::Command;

use crate::build::{Paths, checkout_commit, head_matches, init_checkout, prepend_path, run};

const REPOSITORY: &str = "https://github.com/toncenter/ton-indexer.git";
const COMMIT: &str = "eb9fbfa3212a583d3eef672f74b98600dfdd898c";
const BUILD_SCHEMA: &str = "1";
const SWAG_VERSION: &str = "v1.16.3";
const PATCHES: &[(&str, &str)] = &[
    (
        "ton-indexer-cors.patch",
        include_str!("../../docker/ton-indexer-cors.patch"),
    ),
    (
        "ton-indexer-classifier-cpu.patch",
        include_str!("../../docker/ton-indexer-classifier-cpu.patch"),
    ),
    (
        "ton-indexer-v3-catchup.patch",
        include_str!("../../docker/ton-indexer-v3-catchup.patch"),
    ),
    (
        "ton-indexer-localton-scanner.patch",
        include_str!("../../docker/ton-indexer-localton-scanner.patch"),
    ),
    (
        "ton-indexer-hardfork-accounts.patch",
        include_str!("../../docker/ton-indexer-hardfork-accounts.patch"),
    ),
];

#[derive(Debug, Args)]
pub struct BuildV3Args {
    #[command(flatten)]
    build: crate::build::BuildArgs,

    /// Component to build. The API also requires the worker's native libraries.
    #[arg(long, value_enum, default_value = "all")]
    component: Component,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum Component {
    All,
    Worker,
    Api,
    Classifier,
}

pub async fn build(args: BuildV3Args) -> Result<()> {
    let BuildV3Args {
        build: args,
        component,
    } = args;
    let paths = Paths::new(
        &args.state_dir,
        args.install_dir.as_deref(),
        "ton-http-api-v3",
        "build-worker",
    )?;
    let (repository, commit) = args.source(REPOSITORY, COMMIT, "TON_INDEXER")?;
    prepare_source(&paths, &repository, &commit).await?;
    fs::create_dir_all(&paths.install)?;
    fs::copy(paths.source.join("LICENSE"), paths.install.join("LICENSE"))?;

    match component {
        Component::All => {
            build_worker(&paths, args.jobs).await?;
            build_api(&paths).await?;
            build_classifier(&paths).await?;
        }
        Component::Worker => build_worker(&paths, args.jobs).await?,
        Component::Api => {
            // Docker's API stage inherits the worker artifacts from its parent.
            if !worker_is_installed(&paths)? {
                build_worker(&paths, args.jobs).await?;
            }
            build_api(&paths).await?;
        }
        Component::Classifier => build_classifier(&paths).await?,
    }

    println!("TON Center API V3 source: {commit}");
    println!("installed: {}", paths.install.display());
    Ok(())
}

async fn prepare_source(paths: &Paths, repository: &str, commit: &str) -> Result<()> {
    fs::create_dir_all(&paths.root)?;
    let stamp = paths.root.join(".source-version");
    let expected = serde_json::to_string(&(BUILD_SCHEMA, repository, commit, PATCHES))?;
    init_checkout(&paths.source, repository).await?;
    if head_matches(&paths.source, commit).await?
        && fs::read_to_string(&stamp).is_ok_and(|actual| actual == expected)
    {
        return Ok(());
    }

    // Invalidate before updating so an interrupted checkout is retried next time.
    if stamp.exists() {
        fs::remove_file(&stamp)?;
    }
    checkout_commit(&paths.source, commit).await?;
    run(
        "update TON Indexer submodules",
        Command::new("git").current_dir(&paths.source).args([
            "submodule",
            "update",
            "--init",
            "--recursive",
            "--depth",
            "1",
        ]),
    )
    .await?;

    let patch_dir = paths.root.join("patches");
    fs::create_dir_all(&patch_dir)?;
    for (name, contents) in PATCHES {
        let patch = patch_dir.join(name);
        fs::write(&patch, contents)?;
        run(
            "apply TON Indexer patch",
            Command::new("git")
                .current_dir(&paths.source)
                .arg("apply")
                .arg(&patch),
        )
        .await?;
    }
    if paths.build.exists() {
        fs::remove_dir_all(&paths.build)?;
    }
    fs::write(stamp, expected)?;
    Ok(())
}

fn worker_is_installed(paths: &Paths) -> Result<bool> {
    let expected = fs::read_to_string(paths.root.join(".source-version"))?;
    if !fs::read_to_string(paths.install.join(".worker-version"))
        .is_ok_and(|actual| actual == expected)
        || !paths.install.join("include/wrapper.h").is_file()
    {
        return Ok(false);
    }
    let libraries = marker_libraries(&paths.install.join("lib"))?;
    Ok(["libton-marker.", "libton-marker-core."]
        .iter()
        .all(|prefix| {
            libraries.iter().any(|path| {
                path.is_file()
                    && path
                        .file_name()
                        .unwrap()
                        .to_string_lossy()
                        .starts_with(prefix)
            })
        }))
}

async fn build_worker(paths: &Paths, jobs: u8) -> Result<()> {
    let stamp = paths.install.join(".worker-version");
    if stamp.exists() {
        fs::remove_file(&stamp)?;
    }
    let ccache = env::var_os("CCACHE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| paths.root.join("ccache"));
    fs::create_dir_all(&ccache)?;
    run(
        "configure TON Indexer worker",
        Command::new("cmake")
            .arg("-S")
            .arg(paths.source.join("ton-index-worker"))
            .arg("-B")
            .arg(&paths.build)
            .args([
                "-G",
                "Ninja",
                "-DCMAKE_BUILD_TYPE=Release",
                "-DCMAKE_C_COMPILER_LAUNCHER=ccache",
                "-DCMAKE_CXX_COMPILER_LAUNCHER=ccache",
                "-DPORTABLE=1",
                "-DTON_ARCH=",
            ])
            .env("CCACHE_DIR", &ccache),
    )
    .await?;
    run(
        "build TON Indexer worker",
        Command::new("cmake")
            .arg("--build")
            .arg(&paths.build)
            .args(["--parallel", &jobs.to_string(), "--target"])
            .args([
                "ton-index-postgres",
                "ton-index-postgres-migrate",
                "ton-smc-scanner",
                "ton-marker",
                "ton-marker-cli",
                "ton-marker-core",
            ])
            .env("CCACHE_DIR", &ccache),
    )
    .await?;

    let bin_dir = paths.install.join("bin");
    fs::create_dir_all(&bin_dir)?;
    for (directory, binary) in [
        ("ton-index-postgres", "ton-index-postgres"),
        ("ton-index-postgres", "ton-index-postgres-migrate"),
        ("ton-smc-scanner", "ton-smc-scanner"),
    ] {
        run(
            "install TON Indexer worker binary",
            Command::new("install")
                .args(["-m", "0755"])
                .arg(paths.build.join(directory).join(binary))
                .arg(bin_dir.join(binary)),
        )
        .await?;
    }
    install_marker_libraries(&paths.build.join("ton-marker"), &paths.install.join("lib")).await?;
    fs::create_dir_all(paths.install.join("include"))?;
    fs::copy(
        paths
            .source
            .join("ton-index-worker/ton-marker/src/wrapper.h"),
        paths.install.join("include/wrapper.h"),
    )?;
    fs::copy(paths.root.join(".source-version"), stamp)?;
    Ok(())
}

async fn build_api(paths: &Paths) -> Result<()> {
    let tools_bin = paths.root.join("bin");
    fs::create_dir_all(&tools_bin)?;
    fs::create_dir_all(paths.install.join("bin"))?;
    let source = paths.source.join("ton-index-go");
    run(
        "install TON Indexer OpenAPI generator",
        Command::new("go")
            .current_dir(&source)
            .args([
                "install",
                &format!("github.com/swaggo/swag/cmd/swag@{SWAG_VERSION}"),
            ])
            .env("GOBIN", &tools_bin),
    )
    .await?;
    run(
        "generate TON Indexer OpenAPI documentation",
        Command::new(tools_bin.join("swag"))
            .current_dir(&source)
            .arg("init"),
    )
    .await?;
    let mut command = Command::new("go");
    command
        .current_dir(source)
        .args(["build", "-trimpath", "-ldflags=-s -w -buildid=", "-o"])
        .arg(paths.install.join("bin/ton-index-go"))
        .arg("./main.go")
        .env("CGO_ENABLED", "1");
    prepend_path(&mut command, "CPATH", &paths.install.join("include"))?;
    prepend_path(&mut command, "LIBRARY_PATH", &paths.install.join("lib"))?;
    prepend_path(&mut command, "LD_LIBRARY_PATH", &paths.install.join("lib"))?;
    run("build TON Indexer API", &mut command).await
}

async fn build_classifier(paths: &Paths) -> Result<()> {
    let venv = paths.install.join("venv");
    run(
        "create TON Indexer classifier environment",
        Command::new("python3").args(["-m", "venv"]).arg(&venv),
    )
    .await?;
    run(
        "install TON Indexer classifier dependencies",
        Command::new(venv.join("bin/python"))
            .args(["-m", "pip", "install", "-r"])
            .arg(paths.source.join("indexer/requirements.txt")),
    )
    .await?;
    let classifier = paths.install.join("classifier");
    fs::create_dir_all(&classifier)?;
    run(
        "install TON Indexer classifier",
        Command::new("cp")
            .arg("-a")
            .arg(paths.source.join("indexer/."))
            .arg(classifier),
    )
    .await
}

fn marker_libraries(directory: &Path) -> Result<Vec<PathBuf>> {
    if !directory.exists() {
        return Ok(Vec::new());
    }
    let mut libraries = Vec::new();
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        if entry
            .file_name()
            .to_string_lossy()
            .starts_with("libton-marker")
        {
            libraries.push(entry.path());
        }
    }
    libraries.sort();
    Ok(libraries)
}

async fn install_marker_libraries(source: &Path, destination: &Path) -> Result<()> {
    let libraries = marker_libraries(source)?;
    ensure!(
        !libraries.is_empty(),
        "TON Indexer marker libraries are missing: {}",
        source.display()
    );
    fs::create_dir_all(destination)?;
    run(
        "install TON Indexer marker libraries",
        Command::new("cp")
            .arg("-a")
            .args(libraries)
            .arg(destination),
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn source_rejects_an_existing_directory_without_git() {
        let temp = tempfile::tempdir().unwrap();
        let paths = Paths::new(temp.path(), None, "ton-http-api-v3", "build-worker").unwrap();
        fs::create_dir_all(&paths.source).unwrap();
        fs::write(paths.source.join("keep"), "user data").unwrap();

        let error = prepare_source(&paths, REPOSITORY, COMMIT)
            .await
            .unwrap_err();
        assert!(error.to_string().contains("is not a Git source checkout"));
        assert_eq!(
            fs::read_to_string(paths.source.join("keep")).unwrap(),
            "user data"
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn marker_install_preserves_shared_library_symlinks_and_static_archives() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("build with spaces");
        let destination = temp.path().join("install/lib");
        fs::create_dir(&source).unwrap();
        fs::write(source.join("libton-marker.so.1"), "shared library").unwrap();
        fs::write(source.join("libton-marker-core.a"), "static library").unwrap();
        fs::write(source.join("CMakeCache.txt"), "build only").unwrap();
        std::os::unix::fs::symlink("libton-marker.so.1", source.join("libton-marker.so")).unwrap();

        install_marker_libraries(&source, &destination)
            .await
            .unwrap();

        assert_eq!(
            fs::read_link(destination.join("libton-marker.so")).unwrap(),
            Path::new("libton-marker.so.1")
        );
        assert_eq!(
            fs::read_to_string(destination.join("libton-marker.so")).unwrap(),
            "shared library"
        );
        assert!(destination.join("libton-marker-core.a").is_file());
        assert!(!destination.join("CMakeCache.txt").exists());
    }
}
