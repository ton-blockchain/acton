use anyhow::{Context, Result, bail};
use chrono::{SecondsFormat, Utc};
use clap::{Args, Subcommand};
use semver::Version;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

pub(crate) const TOOLCHAIN_INDEX_PATH: &str = "toolchain-index.json";
const CARGO_TOML_PATH: &str = "Cargo.toml";
const TOLK_VERSION_METADATA_PATH: &str = "workspace.metadata.acton.tolk-version";

const TOOLCHAIN_INDEX_SCHEMA: u32 = 1;

#[derive(Args)]
pub(crate) struct ToolchainIndexArgs {
    #[command(subcommand)]
    command: ToolchainIndexCommand,
}

#[derive(Subcommand)]
enum ToolchainIndexCommand {
    /// Validate toolchain-index.json
    Check,
    /// Add or refresh the release entry for an Acton version
    AddRelease {
        #[arg(long = "version", value_name = "VERSION")]
        version: String,
    },
    /// Mark an existing Acton release as yanked
    Yank {
        #[arg(long = "version", value_name = "VERSION")]
        version: String,
        #[arg(long, value_name = "TEXT")]
        reason: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct ToolchainIndex {
    schema: u32,
    generated_at: String,
    releases: Vec<ToolchainRelease>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct ToolchainRelease {
    acton: String,
    tolk: String,
    tag: String,
    stable: bool,
    #[serde(default)]
    yanked: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    yank_reason: Option<String>,
}

pub(crate) fn run(args: ToolchainIndexArgs) -> Result<()> {
    match args.command {
        ToolchainIndexCommand::Check => {
            check_default_index()?;
            println!("{TOOLCHAIN_INDEX_PATH} is valid");
        }
        ToolchainIndexCommand::AddRelease { version } => {
            let tolk_version = read_canonical_tolk_version()?;
            add_release_to_default_index(&version, &tolk_version)?;
            println!("Updated {TOOLCHAIN_INDEX_PATH} with Acton {version} (Tolk {tolk_version})");
        }
        ToolchainIndexCommand::Yank { version, reason } => {
            yank_release_in_default_index(&version, &reason)?;
            println!("Marked Acton {version} as yanked in {TOOLCHAIN_INDEX_PATH}");
        }
    }

    Ok(())
}

pub(crate) fn check_default_index() -> Result<()> {
    read_index(Path::new(TOOLCHAIN_INDEX_PATH)).map(|_| ())
}

pub(crate) fn add_release_to_default_index(acton_version: &str, tolk_version: &str) -> Result<()> {
    add_release(Path::new(TOOLCHAIN_INDEX_PATH), acton_version, tolk_version)
}

pub(crate) fn read_canonical_tolk_version() -> Result<String> {
    let contents = fs::read_to_string(CARGO_TOML_PATH)
        .with_context(|| format!("failed to read {CARGO_TOML_PATH}"))?;
    let document = contents
        .parse::<toml_edit::DocumentMut>()
        .with_context(|| format!("failed to parse {CARGO_TOML_PATH}"))?;
    let version = document["workspace"]["metadata"]["acton"]["tolk-version"]
        .as_str()
        .with_context(|| format!("missing string `{TOLK_VERSION_METADATA_PATH}`"))?;

    parse_exact_version(TOLK_VERSION_METADATA_PATH, version)?;

    Ok(version.to_owned())
}

fn yank_release_in_default_index(acton_version: &str, reason: &str) -> Result<()> {
    yank_release(Path::new(TOOLCHAIN_INDEX_PATH), acton_version, reason)
}

fn add_release(path: &Path, acton_version: &str, tolk_version: &str) -> Result<()> {
    parse_exact_version("Acton version", acton_version)?;
    parse_exact_version("Tolk version", tolk_version)?;

    let mut index = read_index_or_empty(path)?;
    let tag = format!("v{acton_version}");

    match index
        .releases
        .iter_mut()
        .find(|release| release.acton == acton_version)
    {
        Some(release) if release.tolk != tolk_version => {
            bail!(
                "Acton {acton_version} already exists in {path} with Tolk {}, not {tolk_version}",
                release.tolk,
                path = path.display()
            );
        }
        Some(release) if release.yanked => {
            bail!(
                "Acton {acton_version} already exists in {} and is yanked",
                path.display()
            );
        }
        Some(release) => {
            release.tag = tag;
            release.stable = true;
            release.yanked = false;
            release.yank_reason = None;
        }
        None => index.releases.push(ToolchainRelease {
            acton: acton_version.to_owned(),
            tolk: tolk_version.to_owned(),
            tag,
            stable: true,
            yanked: false,
            yank_reason: None,
        }),
    }

    index.generated_at = current_timestamp();
    sort_releases(&mut index.releases)?;
    validate_index(&index).with_context(|| format!("generated invalid {}", path.display()))?;
    write_index(path, &index)
}

fn yank_release(path: &Path, acton_version: &str, reason: &str) -> Result<()> {
    parse_exact_version("Acton version", acton_version)?;
    let reason = reason.trim();
    if reason.is_empty() {
        bail!("yank reason must not be empty");
    }

    let mut index = read_index(path)?;
    let release = index
        .releases
        .iter_mut()
        .find(|release| release.acton == acton_version)
        .with_context(|| format!("Acton {acton_version} is not present in {}", path.display()))?;

    release.yanked = true;
    release.yank_reason = Some(reason.to_owned());
    index.generated_at = current_timestamp();

    validate_index(&index).with_context(|| format!("generated invalid {}", path.display()))?;
    write_index(path, &index)
}

fn read_index_or_empty(path: &Path) -> Result<ToolchainIndex> {
    if path.exists() {
        return read_index(path);
    }

    Ok(ToolchainIndex {
        schema: TOOLCHAIN_INDEX_SCHEMA,
        generated_at: current_timestamp(),
        releases: Vec::new(),
    })
}

fn read_index(path: &Path) -> Result<ToolchainIndex> {
    let contents =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    let index = serde_json::from_str::<ToolchainIndex>(&contents)
        .with_context(|| format!("failed to parse {}", path.display()))?;

    validate_index(&index).with_context(|| format!("failed to validate {}", path.display()))?;

    Ok(index)
}

fn write_index(path: &Path, index: &ToolchainIndex) -> Result<()> {
    let mut json = serde_json::to_string_pretty(index).context("failed to serialize index")?;
    json.push('\n');
    fs::write(path, json).with_context(|| format!("failed to write {}", path.display()))
}

fn validate_index(index: &ToolchainIndex) -> Result<()> {
    if index.schema != TOOLCHAIN_INDEX_SCHEMA {
        bail!(
            "toolchain index schema must be {TOOLCHAIN_INDEX_SCHEMA}, got {}",
            index.schema
        );
    }

    chrono::DateTime::parse_from_rfc3339(&index.generated_at)
        .with_context(|| format!("generated_at is not RFC 3339: {}", index.generated_at))?;

    let mut seen = BTreeSet::new();
    let mut previous_acton_version = None;

    for release in &index.releases {
        let acton_version = parse_exact_version("release.acton", &release.acton)?;
        parse_exact_version("release.tolk", &release.tolk)?;

        if release.tag != format!("v{}", release.acton) {
            bail!(
                "release {} has tag `{}`, expected `v{}`",
                release.acton,
                release.tag,
                release.acton
            );
        }

        if !seen.insert(acton_version.clone()) {
            bail!("duplicate Acton release {}", release.acton);
        }

        if let Some(previous) = &previous_acton_version
            && acton_version < *previous
        {
            bail!("toolchain releases must be sorted by Acton version");
        }
        previous_acton_version = Some(acton_version);

        if release.yanked {
            let Some(reason) = release.yank_reason.as_deref() else {
                bail!("yanked Acton {} must include yank_reason", release.acton);
            };
            if reason.trim().is_empty() {
                bail!("yanked Acton {} must include yank_reason", release.acton);
            }
        } else if let Some(reason) = release.yank_reason.as_deref()
            && reason.trim().is_empty()
        {
            bail!("Acton {} has an empty yank_reason", release.acton);
        }
    }

    Ok(())
}

fn sort_releases(releases: &mut [ToolchainRelease]) -> Result<()> {
    let mut keyed = releases
        .iter()
        .map(|release| {
            parse_exact_version("release.acton", &release.acton).map(|version| (version, release))
        })
        .collect::<Result<Vec<_>>>()?;

    keyed.sort_by(|(left, _), (right, _)| left.cmp(right));
    let sorted = keyed
        .into_iter()
        .map(|(_, release)| release.clone())
        .collect::<Vec<_>>();

    for (release, sorted_release) in releases.iter_mut().zip(sorted) {
        *release = sorted_release;
    }

    Ok(())
}

fn parse_exact_version(field: &str, value: &str) -> Result<Version> {
    let version =
        Version::parse(value).with_context(|| format!("{field} must be an exact X.Y.Z version"))?;

    if !version.pre.is_empty() || !version.build.is_empty() {
        bail!("{field} must not include pre-release or build metadata: {value}");
    }

    Ok(version)
}

fn current_timestamp() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn write_temp_index(index: &ToolchainIndex) -> (TempDir, std::path::PathBuf) {
        let temp_dir = TempDir::new().expect("temp dir must be created");
        let path = temp_dir.path().join("toolchain-index.json");
        write_index(&path, index).expect("index must be written");
        (temp_dir, path)
    }

    fn sample_index() -> ToolchainIndex {
        ToolchainIndex {
            schema: TOOLCHAIN_INDEX_SCHEMA,
            generated_at: "2026-04-24T00:00:00Z".to_owned(),
            releases: vec![ToolchainRelease {
                acton: "0.3.0".to_owned(),
                tolk: "1.3.0".to_owned(),
                tag: "v0.3.0".to_owned(),
                stable: true,
                yanked: false,
                yank_reason: None,
            }],
        }
    }

    #[test]
    fn validate_index_accepts_exact_versions() {
        validate_index(&sample_index()).expect("index should be valid");
    }

    #[test]
    fn validate_index_rejects_partial_tolk_version() {
        let mut index = sample_index();
        index.releases[0].tolk = "1.3".to_owned();

        let error = validate_index(&index).expect_err("partial Tolk version must fail");

        assert!(error.to_string().contains("release.tolk"));
    }

    #[test]
    fn add_release_sorts_versions_and_preserves_yanked_entries() {
        let mut index = sample_index();
        index.releases[0].yanked = true;
        index.releases[0].yank_reason = Some("broken release".to_owned());
        let (_temp_dir, path) = write_temp_index(&index);

        add_release(&path, "0.2.9", "1.2.0").expect("release must be added");
        let index = read_index(&path).expect("index must be readable");

        assert_eq!(index.releases[0].acton, "0.2.9");
        assert_eq!(index.releases[1].acton, "0.3.0");
        assert!(index.releases[1].yanked);
        assert_eq!(
            index.releases[1].yank_reason.as_deref(),
            Some("broken release")
        );
    }

    #[test]
    fn add_release_rejects_duplicate_with_different_tolk() {
        let (_temp_dir, path) = write_temp_index(&sample_index());

        let error = add_release(&path, "0.3.0", "1.4.0")
            .expect_err("duplicate with different Tolk must fail");

        assert!(error.to_string().contains("already exists"));
    }

    #[test]
    fn yank_release_sets_reason() {
        let (_temp_dir, path) = write_temp_index(&sample_index());

        yank_release(&path, "0.3.0", "broken wrappers").expect("release must be yanked");
        let index = read_index(&path).expect("index must be readable");

        assert!(index.releases[0].yanked);
        assert_eq!(
            index.releases[0].yank_reason.as_deref(),
            Some("broken wrappers")
        );
    }
}
