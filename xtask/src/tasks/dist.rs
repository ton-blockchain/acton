use anyhow::{Context, Result, bail};
use clap::{Args, Subcommand};
use flate2::Compression;
use flate2::write::GzEncoder;
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};

#[derive(Args)]
pub(crate) struct DistArgs {
    #[command(subcommand)]
    command: DistCommand,
}

pub(crate) fn run(args: DistArgs) -> Result<()> {
    match args.command {
        DistCommand::Announcement => run_announcement(),
        DistCommand::Archive(args) => run_archive(args),
        DistCommand::Check => run_check(),
        DistCommand::Installers => run_installers(),
    }
}

#[derive(Subcommand)]
enum DistCommand {
    Announcement,
    Archive(ArchiveArgs),
    Check,
    Installers,
}

#[derive(Args)]
struct ArchiveArgs {
    #[arg(long, value_name = "TARGET")]
    target: String,

    #[arg(long, value_name = "PATH", default_value = "target")]
    target_dir: PathBuf,

    #[arg(long, value_name = "NAME", default_value = "acton")]
    binary_name: String,

    #[arg(long, value_name = "PROFILE", default_value = "release")]
    profile: String,

    #[arg(long, value_name = "DIR")]
    output: Option<PathBuf>,
}

fn run_announcement() -> Result<()> {
    println!(
        "mock dist announcement: TODO: replace print with real release announcement generation",
    );
    Ok(())
}

fn run_archive(args: ArchiveArgs) -> Result<()> {
    let workspace_root =
        std::env::current_dir().context("failed to determine current directory")?;
    let output = create_archive(
        &workspace_root,
        &args.target_dir,
        &args.binary_name,
        &args.target,
        &args.profile,
        args.output.as_deref(),
    )?;
    println!("Archiving {}", output.binary_path.display());
    println!("Created archive {}", output.archive_path.display());
    println!("Created checksum {}", output.checksum_path.display());
    println!("Checksum value {}", output.checksum);

    Ok(())
}

fn run_check() -> Result<()> {
    println!("mock dist check: TODO: replace print with real archive validation",);
    Ok(())
}

fn run_installers() -> Result<()> {
    println!("mock dist installers: TODO: replace print with real installer creation",);
    Ok(())
}

struct ArchiveOutput {
    binary_path: PathBuf,
    archive_path: PathBuf,
    checksum_path: PathBuf,
    checksum: String,
}

fn create_archive(
    workspace_root: &Path,
    target_dir: &Path,
    binary_name: &str,
    target: &str,
    profile: &str,
    output_dir: Option<&Path>,
) -> Result<ArchiveOutput> {
    let output_dir = output_dir
        .map(|path| resolve_path(workspace_root, path))
        .unwrap_or_else(|| workspace_root.to_path_buf());
    ensure_output_dir_exists(&output_dir)?;

    let binary_path = workspace_root
        .join(target_dir)
        .join(target)
        .join(profile)
        .join(binary_name);

    if !binary_path.is_file() {
        bail!(
            "built binary not found at `{}`; run `cargo build --profile {} --locked --target {} --bin {}` first",
            binary_path.display(),
            profile,
            target,
            binary_name
        );
    }

    let archive_name = archive_name(binary_name, target);
    let archive_path = write_archive(&binary_path, &output_dir, &archive_name)?;

    let checksum = compute_sha256(&archive_path)?;
    let checksum_path = checksum_path(&archive_path);
    write_checksum(&archive_path, &checksum, &checksum_path)?;

    Ok(ArchiveOutput {
        binary_path,
        archive_path,
        checksum_path,
        checksum,
    })
}

fn resolve_path(workspace_root: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        workspace_root.join(path)
    }
}

fn archive_name(artifact_name: &str, target: &str) -> String {
    format!("{}-{}{}", artifact_name, target, ".tar.gz")
}

fn checksum_path(archive_path: &Path) -> PathBuf {
    archive_path.with_added_extension("sha256")
}

fn ensure_output_dir_exists(output_dir: &Path) -> Result<()> {
    if output_dir.exists() {
        if output_dir.is_dir() {
            return Ok(());
        }

        bail!(
            "output path `{}` exists and is not a directory",
            output_dir.display()
        );
    }

    fs::create_dir_all(output_dir).with_context(|| {
        format!(
            "failed to create output directory `{}`",
            output_dir.display()
        )
    })
}

fn write_archive(binary_path: &Path, output_dir: &Path, archive_name: &str) -> Result<PathBuf> {
    let archive_path = output_dir.join(archive_name);

    let archive_file = File::create(&archive_path)
        .with_context(|| format!("failed to create archive `{}`", archive_path.display()))?;
    let encoder = GzEncoder::new(archive_file, Compression::default());

    let mut builder = tar::Builder::new(encoder);
    let archive_entry_name = binary_path
        .file_name()
        .with_context(|| format!("failed to get file name from `{}`", binary_path.display()))?;

    builder
        .append_path_with_name(binary_path, archive_entry_name)
        .with_context(|| format!("failed to archive binary `{}`", binary_path.display()))?;

    let encoder = builder
        .into_inner()
        .context("failed to finish writing tar archive")?;
    encoder
        .finish()
        .context("failed to finish writing gzip stream")?;

    Ok(archive_path)
}

fn write_checksum(archive_path: &Path, checksum: &str, checksum_path: &Path) -> Result<()> {
    let archive_name = archive_path
        .file_name()
        .and_then(|name| name.to_str())
        .context("archive path does not have a valid UTF-8 file name")?;
    let contents = format!("{checksum}  {archive_name}\n");

    fs::write(checksum_path, contents).with_context(|| {
        format!(
            "failed to write checksum file `{}`",
            checksum_path.display()
        )
    })
}

fn compute_sha256(path: &Path) -> Result<String> {
    let mut file =
        File::open(path).with_context(|| format!("failed to open `{}`", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 8192];

    loop {
        let read = file
            .read(&mut buffer)
            .with_context(|| format!("failed to read `{}`", path.display()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }

    Ok(format!("{:x}", hasher.finalize()))
}
