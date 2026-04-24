use anyhow::{Context, Result};
use clap::Subcommand;
use fs2::FileExt;
use inquire::Confirm;
use std::fs::{self, OpenOptions};
use std::io::{IsTerminal, stdin, stdout};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

use crate::commands::up::client::{GitHubClient, ReleaseClient};
use crate::commands::up::workflow::{download_verified_release_archive, extract_acton_binary};
use crate::toolchain::{
    CliToolchainSelector, ToolchainEnvironment, ToolchainReleaseMetadata, ToolchainResolveReport,
    installed_toolchain_binary_path, installed_toolchain_dir, load_project_toolchain_config,
    normalize_explicit_acton_version, resolve_toolchain, toolchain_store_dir,
};

const DEFAULT_TOOLCHAIN_LOCK_TIMEOUT: Duration = Duration::from_secs(30);
#[cfg(debug_assertions)]
const TEST_TOOLCHAIN_LOCK_TIMEOUT_MS_ENV: &str = "ACTON_TEST_TOOLCHAIN_LOCK_TIMEOUT_MS";

#[derive(Subcommand, Clone)]
pub enum ToolchainCommand {
    #[command(about = "Install the Acton toolchain selected for this project")]
    Install {
        #[arg(value_name = "ACTON_VERSION")]
        version: Option<String>,
    },
    #[command(about = "Remove an installed side-by-side Acton toolchain")]
    Remove {
        #[arg(value_name = "ACTON_VERSION")]
        version: String,
    },
    #[command(about = "List installed and known Acton project toolchains")]
    List,
    #[command(about = "Print the executable selected for the current project")]
    Which,
    #[command(about = "Resolve the Acton toolchain selected for the current project")]
    Resolve,
}

pub fn toolchain_cmd(command: ToolchainCommand) -> Result<()> {
    match command {
        ToolchainCommand::Install { version } => install_cmd(version),
        ToolchainCommand::Remove { version } => remove_cmd(version),
        ToolchainCommand::List => list_cmd(),
        ToolchainCommand::Which => which_cmd(),
        ToolchainCommand::Resolve => resolve_cmd(),
    }
}

fn install_cmd(version: Option<String>) -> Result<()> {
    let selector = version
        .as_deref()
        .map(normalize_explicit_acton_version)
        .transpose()?
        .map(|acton| CliToolchainSelector { acton });
    let config = if selector.is_none() {
        let config = load_project_toolchain_config()?;
        let Some(config) = config else {
            anyhow::bail!(
                "`acton toolchain install` requires a project [toolchain] section or an explicit Acton version.\nRun `acton toolchain install 0.3.0` or add [toolchain] to Acton.toml."
            );
        };
        Some(config)
    } else {
        None
    };
    let environment = ToolchainEnvironment::runtime()?;
    print_index_warning(&environment);
    let report = resolve_toolchain(config.as_ref(), selector.as_ref(), &environment)?;

    if report.current {
        println!(
            "Current Acton {} already satisfies the selected toolchain (Tolk {}).",
            report.acton, report.tolk
        );
        return Ok(());
    }

    if let Some(path) = report.path.as_deref() {
        println!("Acton {} is already installed at {path}", report.acton);
        return Ok(());
    }

    let installed_path = install_toolchain(&report)?;
    println!(
        "Installed Acton {} (Tolk {}) at {}",
        report.acton,
        report.tolk,
        installed_path.display()
    );

    Ok(())
}

fn remove_cmd(version: String) -> Result<()> {
    let version = normalize_explicit_acton_version(&version)?;
    let target_dir = installed_toolchain_dir(&version)?;
    let target_binary = installed_toolchain_binary_path(&version)?;

    if !target_dir.exists() {
        println!("Acton {version} is not installed.");
        return Ok(());
    }

    if current_exe_is(&target_binary)? {
        anyhow::bail!(
            "Cannot remove Acton {version} because this command is running from {}.",
            target_binary.display()
        );
    }

    if !confirm_toolchain_removal(&version, &target_dir)? {
        println!("Cancelled.");
        return Ok(());
    }

    let store_dir = toolchain_store_dir()?;
    let lock_path = store_dir.join(format!(".{version}.lock"));
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&lock_path)?;
    lock_toolchain_version(&lock, &version)?;

    let result = remove_toolchain_locked(&version, &target_dir);
    let _ = lock.unlock();
    result
}

fn list_cmd() -> Result<()> {
    let environment = ToolchainEnvironment::runtime()?;
    print_index_warning(&environment);

    if environment.installed.is_empty() {
        println!("Installed toolchains: none");
    } else {
        println!("Installed toolchains:");
        for (version, installed) in &environment.installed {
            println!("  {version}  {}", installed.binary_path.display());
        }
    }

    if let Some(index) = environment.index.as_ref() {
        println!();
        println!("Known toolchains:");
        for release in index.releases() {
            let status = if release.yanked {
                "yanked"
            } else if release.stable {
                "stable"
            } else {
                "unstable"
            };
            if let Some(reason) = release.yank_reason.as_deref() {
                println!(
                    "  {}  Tolk {}  {} ({})",
                    release.acton, release.tolk, status, reason
                );
            } else {
                println!("  {}  Tolk {}  {}", release.acton, release.tolk, status);
            }
        }
    }

    Ok(())
}

fn which_cmd() -> Result<()> {
    let config = load_project_toolchain_config()?;
    let environment = ToolchainEnvironment::runtime()?;
    let report = resolve_toolchain(config.as_ref(), None, &environment)?;

    if let Some(path) = report.path.as_deref() {
        println!("{path}");
        return Ok(());
    }

    anyhow::bail!(
        "Acton {} (Tolk {}) is selected but not installed.\nRun `acton toolchain install` from the project root or `acton toolchain install {}`.",
        report.acton,
        report.tolk,
        report.acton
    );
}

fn resolve_cmd() -> Result<()> {
    let config = load_project_toolchain_config()?;
    let environment = ToolchainEnvironment::runtime()?;
    print_index_warning(&environment);
    let report = resolve_toolchain(config.as_ref(), None, &environment)?;

    println!("{}", serde_json::to_string_pretty(&report)?);

    Ok(())
}

pub fn install_toolchain(report: &ToolchainResolveReport) -> Result<PathBuf> {
    let target_dir = installed_toolchain_dir(&report.acton)?;
    let target_binary = installed_toolchain_binary_path(&report.acton)?;
    let store_dir = toolchain_store_dir()?;
    fs::create_dir_all(&store_dir)?;

    let lock_path = store_dir.join(format!(".{}.lock", report.acton));
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&lock_path)?;
    lock_toolchain_version(&lock, &report.acton)?;

    let result = install_toolchain_locked(report, &target_dir, &target_binary, &store_dir);
    let _ = lock.unlock();
    result
}

fn install_toolchain_locked(
    report: &ToolchainResolveReport,
    target_dir: &Path,
    target_binary: &Path,
    store_dir: &Path,
) -> Result<PathBuf> {
    if target_binary.is_file() {
        return Ok(target_binary.to_path_buf());
    }

    if target_dir.exists() {
        anyhow::bail!(
            "Toolchain directory {} already exists but does not contain an Acton binary.\nRun `acton toolchain remove {}` and reinstall it.",
            target_dir.display(),
            report.acton
        );
    }

    let token = std::env::var("GITHUB_TOKEN").ok();
    let client = GitHubClient::new(token);
    let release = client.get_release(Some(&report.acton), false)?;
    let archive = download_verified_release_archive(&client, &release)?;

    let install_result = (|| {
        let extracted = extract_acton_binary(&archive.path)?;

        let temp_dir = tempfile::Builder::new()
            .prefix(&format!(".{}.", report.acton))
            .tempdir_in(store_dir)?;
        let temp_binary = temp_dir.path().join(acton_binary_name());
        fs::copy(&extracted.path, &temp_binary)?;
        set_executable_permissions(&temp_binary)?;
        probe_installed_toolchain(&temp_binary, report)?;
        write_release_metadata(temp_dir.path(), report)?;

        let temp_path = temp_dir.keep();
        fs::rename(&temp_path, target_dir)?;

        Ok(target_binary.to_path_buf())
    })();

    let _ = fs::remove_file(&archive.path);

    install_result
}

fn remove_toolchain_locked(version: &str, target_dir: &Path) -> Result<()> {
    if !target_dir.exists() {
        println!("Acton {version} is not installed.");
        return Ok(());
    }

    fs::remove_dir_all(target_dir).with_context(|| {
        format!(
            "Failed to remove toolchain directory {}",
            target_dir.display()
        )
    })?;
    println!("Removed Acton {version} from {}", target_dir.display());
    Ok(())
}

fn confirm_toolchain_removal(version: &str, target_dir: &Path) -> Result<bool> {
    if !stdin().is_terminal() || !stdout().is_terminal() {
        anyhow::bail!(
            "Confirmation required to remove Acton {version} at {}.\nRun this command in an interactive terminal.",
            target_dir.display()
        );
    }

    Confirm::new(&format!(
        "Remove Acton {version} from {}?",
        target_dir.display()
    ))
    .with_default(false)
    .prompt()
    .context("Failed to read toolchain removal confirmation")
}

fn print_index_warning(environment: &ToolchainEnvironment) {
    if let Some(warning) = environment.index_warning.as_deref() {
        eprintln!("Warning: {warning}");
    }
}

fn lock_toolchain_version(lock: &fs::File, version: &str) -> Result<()> {
    let timeout = toolchain_lock_timeout();
    let deadline = Instant::now() + timeout;

    loop {
        match lock.try_lock_exclusive() {
            Ok(()) => return Ok(()),
            Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                if Instant::now() >= deadline {
                    anyhow::bail!(
                        "Another Acton process is installing {version} and the install lock timed out.\nTry again after the other process exits."
                    );
                }
                thread::sleep(Duration::from_millis(50));
            }
            Err(err) => {
                return Err(err)
                    .with_context(|| format!("Failed to lock toolchain install for {version}"));
            }
        }
    }
}

fn toolchain_lock_timeout() -> Duration {
    #[cfg(debug_assertions)]
    if let Ok(value) = std::env::var(TEST_TOOLCHAIN_LOCK_TIMEOUT_MS_ENV)
        && let Ok(millis) = value.parse::<u64>()
    {
        return Duration::from_millis(millis);
    }

    DEFAULT_TOOLCHAIN_LOCK_TIMEOUT
}

fn probe_installed_toolchain(binary: &Path, report: &ToolchainResolveReport) -> Result<()> {
    let output = Command::new(binary).arg("-V").output().with_context(|| {
        format!(
            "Failed to execute installed toolchain at {}",
            binary.display()
        )
    })?;

    if !output.status.success() {
        anyhow::bail!(
            "Installed toolchain at {} failed version probe with status {}.\nRun `acton toolchain remove {}` and then `acton toolchain install {}`.",
            binary.display(),
            output.status,
            report.acton,
            report.acton
        );
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let (acton, tolk) = parse_version_probe_output(&stdout).with_context(|| {
        format!(
            "Installed toolchain at {} did not report Acton and Tolk versions",
            binary.display()
        )
    })?;

    if acton != report.acton {
        anyhow::bail!(
            "Installed toolchain at {} reports Acton {}.\nRun `acton toolchain remove {}` and then `acton toolchain install {}`.",
            binary.display(),
            acton,
            report.acton,
            report.acton
        );
    }

    if tolk != report.tolk {
        anyhow::bail!(
            "Acton {} was selected for Tolk {}, but {} reports Tolk {}.\nUpdate the toolchain index or reinstall the toolchain.",
            report.acton,
            report.tolk,
            binary.display(),
            tolk
        );
    }

    Ok(())
}

fn parse_version_probe_output(stdout: &str) -> Result<(String, String)> {
    let text = stdout.trim();
    let Some(rest) = text.strip_prefix("acton ") else {
        anyhow::bail!("missing `acton` prefix");
    };
    let Some((acton, tolk)) = rest.split_once(" with Tolk ") else {
        anyhow::bail!("missing bundled Tolk version");
    };
    Ok((acton.trim().to_owned(), tolk.trim().to_owned()))
}

fn current_exe_is(path: &Path) -> Result<bool> {
    let current_exe = std::env::current_exe().context("failed to resolve current executable")?;
    if current_exe == path {
        return Ok(true);
    }

    let Ok(current_exe) = dunce::canonicalize(current_exe) else {
        return Ok(false);
    };
    let Ok(path) = dunce::canonicalize(path) else {
        return Ok(false);
    };
    Ok(current_exe == path)
}

fn write_release_metadata(path: &Path, report: &ToolchainResolveReport) -> Result<()> {
    let metadata = ToolchainReleaseMetadata {
        schema: 1,
        acton: report.acton.clone(),
        tolk: report.tolk.clone(),
        target_triple: env!("TARGET_TRIPLE").to_owned(),
        yanked: report.yanked,
        yank_reason: report.yank_reason.clone(),
    };
    let mut json = serde_json::to_string_pretty(&metadata)?;
    json.push('\n');
    fs::write(path.join("release.json"), json)?;
    Ok(())
}

fn set_executable_permissions(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(path)?.permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions)?;
    }

    Ok(())
}

const fn acton_binary_name() -> &'static str {
    if cfg!(windows) { "acton.exe" } else { "acton" }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_metadata_includes_selected_versions() {
        let report = ToolchainResolveReport {
            source: "project-acton",
            acton: "0.3.0".to_owned(),
            tolk: "1.3.0".to_owned(),
            current: false,
            installed: false,
            install_required: true,
            path: None,
            yanked: false,
            yank_reason: None,
        };
        let metadata = ToolchainReleaseMetadata {
            schema: 1,
            acton: report.acton.clone(),
            tolk: report.tolk.clone(),
            target_triple: "test-target".to_owned(),
            yanked: report.yanked,
            yank_reason: report.yank_reason,
        };

        let json = serde_json::to_string(&metadata).expect("metadata must serialize");

        assert!(json.contains("\"acton\":\"0.3.0\""));
        assert!(json.contains("\"tolk\":\"1.3.0\""));
        assert!(json.contains("\"target_triple\":\"test-target\""));
    }

    #[test]
    fn parses_version_probe_output() {
        let (acton, tolk) = parse_version_probe_output("acton 0.3.0 with Tolk 1.3.0\n").unwrap();

        assert_eq!(acton, "0.3.0");
        assert_eq!(tolk, "1.3.0");
    }
}
