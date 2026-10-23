use crate::http;
use acton_config::color::OwoColorize;
use anyhow::{Context, anyhow};
use indicatif::{ProgressBar, ProgressStyle};
use inquire::Confirm;
use sha2::{Digest, Sha256};
use std::env;
use std::fs;
use std::io::{IsTerminal, Read, Write, stderr, stdin, stdout};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

// Acton selects the TSA release used by its bundled checker intrinsics.
const TSA_VERSION: &str = "v0.5.5";
const TSA_RELEASE_URL: &str =
    "https://github.com/espritoxyz/tsa/releases/download/v0.5.5/tsa-cli.jar";
const TSA_RELEASE_SIZE: u64 = 87_678_987;
const TSA_RELEASE_SHA256: &str = "ec03e6af31a8e4ff9d2ee3e66f4dd2b3081c61cda1a1f92f7516be56ee38873f";

struct ReleaseSource {
    url: String,
    size: u64,
    sha256: String,
}

pub(super) fn resolve_tsa_jar(
    explicit_path: Option<&str>,
    yes: bool,
    project_root: &Path,
) -> anyhow::Result<PathBuf> {
    let configured_path = env::var("ACTON_TSA_JAR").ok();
    if let Some(path) = explicit_path.or(configured_path.as_deref()) {
        return super::resolve_existing_path(path);
    }

    let jar_path = project_root
        .join(".acton")
        .join("tsa")
        .join(format!("tsa-cli-{TSA_VERSION}.jar"));
    if jar_path.is_file() {
        return Ok(jar_path);
    }

    if !yes {
        if !stdin().is_terminal() || !stdout().is_terminal() {
            anyhow::bail!(
                "TSA {TSA_VERSION} is not installed at {}. Run again with --yes to download it, or pass --tsa-cli <path> (or ACTON_TSA_JAR)",
                jar_path.display()
            );
        }

        let question = format!(
            "Download TSA {TSA_VERSION} (84 MiB) from GitHub to {}?",
            jar_path.display()
        );
        if !Confirm::new(&question)
            .with_default(true)
            .prompt()
            .context("Failed to confirm TSA installation")?
        {
            anyhow::bail!(
                "TSA installation was declined. Pass --tsa-cli <path> or set ACTON_TSA_JAR to use an existing JAR"
            );
        }
    }

    install_tsa_jar(&jar_path)?;
    Ok(jar_path)
}

fn install_tsa_jar(jar_path: &Path) -> anyhow::Result<()> {
    let started = Instant::now();
    log::info!(
        "operation=tsa.install target={} outcome=started",
        jar_path.display()
    );

    let result = download_tsa_jar(jar_path);
    match &result {
        Ok(()) => log::info!(
            "operation=tsa.install target={} duration_ms={} outcome=success",
            jar_path.display(),
            started.elapsed().as_millis()
        ),
        Err(error) => log::error!(
            "operation=tsa.install target={} duration_ms={} outcome=failure error={error:#}",
            jar_path.display(),
            started.elapsed().as_millis()
        ),
    }

    result
}

fn download_tsa_jar(jar_path: &Path) -> anyhow::Result<()> {
    let source = release_source()?;
    let parent = jar_path.parent().ok_or_else(|| {
        anyhow!(
            "TSA installation path has no parent: {}",
            jar_path.display()
        )
    })?;
    fs::create_dir_all(parent)
        .with_context(|| format!("Failed to create TSA directory {}", parent.display()))?;

    println!(
        "{} TSA {TSA_VERSION} from GitHub releases",
        "Downloading".green().bold()
    );
    let client = http::blocking_client_builder()
        .timeout(Duration::from_secs(600))
        .build()
        .context("Failed to create TSA download client")?;
    let mut response = client
        .get(&source.url)
        .send()
        .with_context(|| format!("Failed to download TSA from {}", source.url))?
        .error_for_status()
        .with_context(|| format!("TSA release download failed at {}", source.url))?;

    let progress = if stderr().is_terminal() {
        ProgressBar::new(source.size)
    } else {
        ProgressBar::hidden()
    };
    progress.set_style(
        ProgressStyle::default_bar()
            .template(" {bar:40} {bytes}/{total_bytes} ({eta})")
            .context("Failed to configure TSA download progress")?,
    );

    let mut temporary = tempfile::NamedTempFile::new_in(parent).with_context(|| {
        format!(
            "Failed to create a temporary TSA JAR in {}",
            parent.display()
        )
    })?;
    let mut digest = Sha256::new();
    let mut downloaded = 0_u64;
    let mut buffer = [0_u8; 32 * 1024];

    loop {
        let length = response
            .read(&mut buffer)
            .with_context(|| format!("Failed while downloading TSA from {}", source.url))?;
        if length == 0 {
            break;
        }

        temporary.write_all(&buffer[..length]).with_context(|| {
            format!(
                "Failed to write temporary TSA JAR for {}",
                jar_path.display()
            )
        })?;
        digest.update(&buffer[..length]);
        downloaded += length as u64;
        if downloaded > source.size {
            anyhow::bail!(
                "TSA download exceeded the expected size of {} bytes from {}",
                source.size,
                source.url
            );
        }
        progress.set_position(downloaded);
    }
    progress.finish_and_clear();

    if downloaded != source.size {
        anyhow::bail!(
            "TSA download was incomplete: expected {} bytes, received {downloaded} bytes from {}",
            source.size,
            source.url
        );
    }
    let actual_sha256 = hex::encode(digest.finalize());
    if actual_sha256 != source.sha256 {
        anyhow::bail!(
            "TSA download checksum mismatch for {}: expected {}, got {actual_sha256}",
            source.url,
            source.sha256
        );
    }

    temporary.as_file_mut().sync_all().with_context(|| {
        format!(
            "Failed to flush temporary TSA JAR for {}",
            jar_path.display()
        )
    })?;
    match temporary.persist_noclobber(jar_path) {
        Ok(_) => {}
        Err(error) if jar_path.is_file() => {
            // Another Acton process completed the same installation first.
            drop(error);
        }
        Err(error) => {
            return Err(error.error)
                .with_context(|| format!("Failed to install TSA JAR at {}", jar_path.display()));
        }
    }

    println!(
        "{} TSA {TSA_VERSION} at {}",
        "Installed".green().bold(),
        jar_path.display()
    );
    Ok(())
}

fn release_source() -> anyhow::Result<ReleaseSource> {
    #[cfg(debug_assertions)]
    if let Ok(url) = env::var("ACTON_TEST_TSA_DOWNLOAD_URL") {
        let size = env::var("ACTON_TEST_TSA_DOWNLOAD_SIZE")?
            .parse()
            .context("Invalid ACTON_TEST_TSA_DOWNLOAD_SIZE")?;
        let sha256 = env::var("ACTON_TEST_TSA_DOWNLOAD_SHA256")?;
        return Ok(ReleaseSource { url, size, sha256 });
    }

    Ok(ReleaseSource {
        url: TSA_RELEASE_URL.to_owned(),
        size: TSA_RELEASE_SIZE,
        sha256: TSA_RELEASE_SHA256.to_owned(),
    })
}
