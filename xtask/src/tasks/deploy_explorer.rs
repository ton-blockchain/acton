use anyhow::{Context, Result, bail};
use clap::Args;
use std::env;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use serde::Deserialize;
use url::Url;

const DEFAULT_REPOSITORY: &str = "https://github.com/i582/actonscan";
const DEFAULT_BRANCH: &str = "pages";
const DEFAULT_CHECKOUT_DIR: &str = "target/actonscan-pages";
const DEPLOYMENT_POLL_INTERVAL: Duration = Duration::from_secs(5);
const DEPLOYMENT_WAIT_TIMEOUT: Duration = Duration::from_secs(10 * 60);
const EXPLORER_PACKAGE: &str = "@acton/explorer-ui";
const EXPLORER_PACKAGE_DIR: &str = "packages/explorer-ui";
const EXPLORER_TONCENTER_API_KEY_ENV: &str = "EXPLORER_TONCENTER_API_KEY";
const VITE_EXPLORER_TONCENTER_API_KEY_ENV: &str = "VITE_EXPLORER_TONCENTER_API_KEY";
const CLOUDFLARE_CHECK_NAME: &str = "Cloudflare Pages";
const CLOUDFLARE_GITHUB_APP: &str = "cloudflare-workers-and-pages";

#[derive(Args)]
pub(crate) struct DeployExplorerArgs {
    #[arg(long, value_name = "URL", default_value = DEFAULT_REPOSITORY)]
    repository: String,

    #[arg(long, value_name = "BRANCH", default_value = DEFAULT_BRANCH)]
    branch: String,

    #[arg(long, value_name = "PATH", default_value = DEFAULT_CHECKOUT_DIR)]
    checkout: PathBuf,

    #[arg(long, value_name = "DOMAIN_OR_URL")]
    cname: Option<String>,

    #[arg(long, value_name = "MESSAGE", default_value = "Deploy actonscan")]
    message: String,

    /// Wait for the Cloudflare Pages check run on the deployed commit.
    #[arg(long)]
    wait_for_deployment: bool,
}

pub(crate) fn run(args: DeployExplorerArgs) -> Result<()> {
    let workspace_root = workspace_root()?;
    let checkout_dir = resolve_path(&workspace_root, &args.checkout);
    let dist_dir = workspace_root.join(EXPLORER_PACKAGE_DIR).join("dist");
    let cname = normalize_cname(args.cname.as_deref())?;

    println!("Building `{EXPLORER_PACKAGE}`");
    let mut build_command = Command::new("bun");
    build_command
        .arg("--filter")
        .arg(EXPLORER_PACKAGE)
        .arg("build")
        .current_dir(&workspace_root);
    if let Some(api_key) =
        env::var_os(EXPLORER_TONCENTER_API_KEY_ENV).filter(|value| !value.is_empty())
    {
        build_command.env(VITE_EXPLORER_TONCENTER_API_KEY_ENV, api_key);
    }
    run_inherited(&mut build_command)?;

    ensure_dist_ready(&dist_dir)?;
    ensure_checkout(&checkout_dir, &args.repository)?;
    prepare_branch(&checkout_dir, &args.branch)?;
    sync_dist(&dist_dir, &checkout_dir, cname.as_deref())?;
    let deployed_commit = commit_and_push(&checkout_dir, &args.branch, &args.message)?;

    if let Some(commit) = deployed_commit {
        println!(
            "Explorer deploy pushed to `{}` branch `{}`",
            args.repository, args.branch
        );
        if let Some(url) = github_commit_url(&args.repository, &commit) {
            println!("Deployed commit: {url}");
        } else {
            println!("Deployed commit: {commit}");
        }
        if args.wait_for_deployment {
            wait_for_deployment(
                &args.repository,
                &commit,
                DEPLOYMENT_WAIT_TIMEOUT,
                DEPLOYMENT_POLL_INTERVAL,
            )?;
        }
    }
    Ok(())
}

fn normalize_cname(value: Option<&str>) -> Result<Option<String>> {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };

    if !value.contains("://") {
        return Ok(Some(value.to_owned()));
    }

    let url = Url::parse(value).with_context(|| format!("invalid CNAME URL `{value}`"))?;
    let host = url
        .host_str()
        .with_context(|| format!("CNAME URL `{value}` does not contain a host"))?;
    Ok(Some(host.to_owned()))
}

fn workspace_root() -> Result<PathBuf> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .context("failed to resolve workspace root from xtask manifest directory")
}

fn resolve_path(workspace_root: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        workspace_root.join(path)
    }
}

fn ensure_dist_ready(dist_dir: &Path) -> Result<()> {
    let index_html = dist_dir.join("index.html");
    if !index_html.is_file() {
        bail!(
            "explorer build output is missing `{}`",
            index_html.display()
        );
    }

    Ok(())
}

fn ensure_checkout(checkout_dir: &Path, repository: &str) -> Result<()> {
    if checkout_dir.join(".git").is_dir() {
        println!(
            "Using existing deploy checkout `{}`",
            checkout_dir.display()
        );
        return Ok(());
    }

    if checkout_dir.exists() {
        bail!(
            "deploy checkout path `{}` exists but is not a git repository",
            checkout_dir.display()
        );
    }

    if let Some(parent) = checkout_dir.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create `{}`", parent.display()))?;
    }

    println!("Cloning `{repository}` into `{}`", checkout_dir.display());
    run_inherited(
        Command::new("git")
            .arg("clone")
            .arg(repository)
            .arg(checkout_dir),
    )
}

fn prepare_branch(checkout_dir: &Path, branch: &str) -> Result<()> {
    run_inherited(git(checkout_dir).arg("fetch").arg("origin"))?;

    if remote_branch_exists(checkout_dir, branch)? {
        println!("Checking out existing deploy branch `{branch}`");
        run_inherited(git(checkout_dir).arg("checkout").arg(branch))?;
        run_inherited(
            git(checkout_dir)
                .arg("reset")
                .arg("--hard")
                .arg(format!("origin/{branch}")),
        )?;
    } else {
        println!("Creating orphan deploy branch `{branch}`");
        run_inherited(
            git(checkout_dir)
                .arg("checkout")
                .arg("--orphan")
                .arg(branch),
        )?;
    }

    clean_checkout_contents(checkout_dir)
}

fn remote_branch_exists(checkout_dir: &Path, branch: &str) -> Result<bool> {
    let output = git(checkout_dir)
        .arg("ls-remote")
        .arg("--heads")
        .arg("origin")
        .arg(branch)
        .output()
        .with_context(|| format!("failed to check remote branch `{branch}`"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!(
            "git ls-remote failed with status {}: {}",
            output.status,
            stderr.trim()
        );
    }

    Ok(!output.stdout.is_empty())
}

fn clean_checkout_contents(checkout_dir: &Path) -> Result<()> {
    for entry in fs::read_dir(checkout_dir)
        .with_context(|| format!("failed to read `{}`", checkout_dir.display()))?
    {
        let entry = entry
            .with_context(|| format!("failed to read entry in `{}`", checkout_dir.display()))?;
        if entry.file_name() == OsStr::new(".git") {
            continue;
        }

        remove_path(&entry.path())?;
    }

    Ok(())
}

fn remove_path(path: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(path)
        .with_context(|| format!("failed to stat `{}`", path.display()))?;
    if metadata.is_dir() {
        fs::remove_dir_all(path)
            .with_context(|| format!("failed to remove directory `{}`", path.display()))?;
    } else {
        fs::remove_file(path).with_context(|| format!("failed to remove `{}`", path.display()))?;
    }

    Ok(())
}

fn sync_dist(dist_dir: &Path, checkout_dir: &Path, cname: Option<&str>) -> Result<()> {
    copy_dir_recursive(dist_dir, checkout_dir)?;
    fs::write(checkout_dir.join(".nojekyll"), "").with_context(|| {
        format!(
            "failed to write `{}`",
            checkout_dir.join(".nojekyll").display()
        )
    })?;

    if let Some(cname) = cname.map(str::trim).filter(|value| !value.is_empty()) {
        fs::write(checkout_dir.join("CNAME"), format!("{cname}\n")).with_context(|| {
            format!("failed to write `{}`", checkout_dir.join("CNAME").display())
        })?;
    }

    Ok(())
}

fn copy_dir_recursive(from: &Path, to: &Path) -> Result<()> {
    fs::create_dir_all(to).with_context(|| format!("failed to create `{}`", to.display()))?;

    for entry in
        fs::read_dir(from).with_context(|| format!("failed to read `{}`", from.display()))?
    {
        let entry =
            entry.with_context(|| format!("failed to read entry in `{}`", from.display()))?;
        let from_path = entry.path();
        let to_path = to.join(entry.file_name());
        let file_type = entry
            .file_type()
            .with_context(|| format!("failed to read file type for `{}`", from_path.display()))?;

        if file_type.is_dir() {
            copy_dir_recursive(&from_path, &to_path)?;
        } else {
            fs::copy(&from_path, &to_path).with_context(|| {
                format!(
                    "failed to copy `{}` to `{}`",
                    from_path.display(),
                    to_path.display()
                )
            })?;
        }
    }

    Ok(())
}

fn commit_and_push(checkout_dir: &Path, branch: &str, message: &str) -> Result<Option<String>> {
    run_inherited(git(checkout_dir).arg("add").arg("-A"))?;

    if !has_staged_changes(checkout_dir)? {
        println!("Deploy checkout has no changes; skipping commit and push");
        return Ok(None);
    }

    run_inherited(git(checkout_dir).arg("commit").arg("-m").arg(message))?;
    let commit = current_commit(checkout_dir)?;
    run_inherited(
        git(checkout_dir)
            .arg("push")
            .arg("origin")
            .arg(format!("HEAD:{branch}")),
    )?;
    Ok(Some(commit))
}

fn current_commit(checkout_dir: &Path) -> Result<String> {
    let output = git(checkout_dir)
        .arg("rev-parse")
        .arg("HEAD")
        .output()
        .context("failed to read deployed commit SHA")?;
    if !output.status.success() {
        bail!("git rev-parse HEAD failed with status {}", output.status);
    }

    let commit = String::from_utf8(output.stdout).context("deployed commit SHA is not UTF-8")?;
    let commit = commit.trim();
    if commit.is_empty() {
        bail!("git rev-parse HEAD returned an empty commit SHA");
    }
    Ok(commit.to_owned())
}

fn github_commit_url(repository: &str, commit: &str) -> Option<String> {
    let repository_path = github_repository_path(repository)?;

    Some(format!(
        "https://github.com/{repository_path}/commit/{commit}"
    ))
}

fn github_repository_path(repository: &str) -> Option<String> {
    let repository_path = if let Some(path) = repository.strip_prefix("git@github.com:") {
        path.to_owned()
    } else {
        let url = Url::parse(repository).ok()?;
        if url.host_str()? != "github.com" {
            return None;
        }
        url.path().trim_start_matches('/').to_owned()
    };
    let repository_path = repository_path.trim_end_matches('/');
    let repository_path = repository_path
        .strip_suffix(".git")
        .unwrap_or(repository_path);
    if repository_path.is_empty() {
        return None;
    }

    Some(repository_path.to_owned())
}

fn wait_for_deployment(
    repository: &str,
    commit: &str,
    timeout: Duration,
    poll_interval: Duration,
) -> Result<()> {
    let repository_path = github_repository_path(repository).with_context(|| {
        format!("cannot wait for deployment of non-GitHub repository `{repository}`")
    })?;
    let started_at = Instant::now();
    let mut check_seen = false;

    println!(
        "Waiting up to {} seconds for Cloudflare Pages deployment of `{commit}`",
        timeout.as_secs()
    );

    loop {
        match cloudflare_deployment_check(&repository_path, commit)? {
            None => {}
            Some(check) if check.status != "completed" => {
                if !check_seen {
                    println!(
                        "Cloudflare Pages deployment started{}",
                        details_suffix(check.details_url.as_deref())
                    );
                    check_seen = true;
                }
            }
            Some(check) if check.conclusion.as_deref() == Some("success") => {
                println!(
                    "Cloudflare Pages deployment succeeded{}",
                    details_suffix(check.details_url.as_deref())
                );
                return Ok(());
            }
            Some(check) => {
                let conclusion = check.conclusion.as_deref().unwrap_or("<none>");
                bail!(
                    "Cloudflare Pages deployment concluded with `{conclusion}`{}",
                    details_suffix(check.details_url.as_deref())
                );
            }
        }

        if started_at.elapsed() >= timeout {
            let state = if check_seen {
                "to complete"
            } else {
                "to appear"
            };
            bail!(
                "timed out after {} seconds waiting for Cloudflare Pages deployment {state} for `{commit}`",
                timeout.as_secs()
            );
        }

        thread::sleep(poll_interval);
    }
}

fn cloudflare_deployment_check(repository_path: &str, commit: &str) -> Result<Option<CheckRun>> {
    let endpoint = format!("repos/{repository_path}/commits/{commit}/check-runs");
    let output = Command::new("gh")
        .args([
            "api",
            "-H",
            "Accept: application/vnd.github+json",
            &endpoint,
        ])
        .output()
        .with_context(|| format!("failed to query GitHub check runs for `{commit}`"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        bail!(
            "gh api {endpoint} failed with status {}: {stderr}",
            output.status
        );
    }

    let response = serde_json::from_slice::<CheckRunsResponse>(&output.stdout)
        .context("failed to parse GitHub check runs response")?;
    Ok(find_cloudflare_deployment_check(response.check_runs))
}

fn find_cloudflare_deployment_check(check_runs: Vec<CheckRun>) -> Option<CheckRun> {
    check_runs
        .into_iter()
        .filter(|check| {
            check.name == CLOUDFLARE_CHECK_NAME && check.app.slug == CLOUDFLARE_GITHUB_APP
        })
        .max_by_key(|check| check.id)
}

fn details_suffix(details_url: Option<&str>) -> String {
    details_url
        .map(|url| format!(": {url}"))
        .unwrap_or_default()
}

fn has_staged_changes(checkout_dir: &Path) -> Result<bool> {
    let status = git(checkout_dir)
        .arg("diff")
        .arg("--cached")
        .arg("--quiet")
        .status()
        .context("failed to run git diff --cached --quiet")?;

    match status.code() {
        Some(0) => Ok(false),
        Some(1) => Ok(true),
        _ => bail!("git diff --cached --quiet failed with status {status}"),
    }
}

#[derive(Debug, Deserialize)]
struct CheckRunsResponse {
    check_runs: Vec<CheckRun>,
}

#[derive(Debug, Deserialize)]
struct CheckRun {
    id: u64,
    name: String,
    status: String,
    conclusion: Option<String>,
    details_url: Option<String>,
    app: CheckRunApp,
}

#[derive(Debug, Deserialize)]
struct CheckRunApp {
    slug: String,
}

fn git(checkout_dir: &Path) -> Command {
    let mut command = Command::new("git");
    command.arg("-C").arg(checkout_dir);
    command
}

fn run_inherited(command: &mut Command) -> Result<()> {
    let program = command.get_program().to_string_lossy().into_owned();
    let args = command
        .get_args()
        .map(|arg| arg.to_string_lossy())
        .collect::<Vec<_>>()
        .join(" ");
    let status = command
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .with_context(|| format!("failed to run `{program} {args}`"))?;

    if !status.success() {
        bail!("`{program} {args}` failed with status {status}");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        CheckRun, CheckRunApp, find_cloudflare_deployment_check, github_commit_url,
        github_repository_path, normalize_cname,
    };

    #[test]
    fn cname_keeps_bare_domain() {
        assert_eq!(
            normalize_cname(Some(" actonscan.com ")).unwrap(),
            Some("actonscan.com".to_owned())
        );
    }

    #[test]
    fn cname_extracts_host_from_url() {
        assert_eq!(
            normalize_cname(Some("https://actonscan.com/explorer")).unwrap(),
            Some("actonscan.com".to_owned())
        );
    }

    #[test]
    fn cname_ignores_empty_value() {
        assert_eq!(normalize_cname(Some("  ")).unwrap(), None);
    }

    #[test]
    fn cname_rejects_url_without_host() {
        let error = normalize_cname(Some("file:///explorer")).unwrap_err();
        assert!(error.to_string().contains("does not contain a host"));
    }

    #[test]
    fn commit_url_supports_https_repository() {
        assert_eq!(
            github_commit_url("https://github.com/i582/actonscan", "abc123").as_deref(),
            Some("https://github.com/i582/actonscan/commit/abc123")
        );
    }

    #[test]
    fn commit_url_supports_ssh_repository() {
        assert_eq!(
            github_commit_url("git@github.com:i582/actonscan.git", "abc123").as_deref(),
            Some("https://github.com/i582/actonscan/commit/abc123")
        );
    }

    #[test]
    fn repository_path_rejects_non_github_repository() {
        assert_eq!(
            github_repository_path("https://gitlab.com/i582/actonscan"),
            None
        );
    }

    #[test]
    fn deployment_check_selects_latest_cloudflare_pages_check() {
        let checks = vec![
            check_run(1, "build", "github-actions"),
            check_run(2, "Cloudflare Pages", "cloudflare-workers-and-pages"),
            check_run(3, "Cloudflare Pages", "cloudflare-workers-and-pages"),
        ];

        assert_eq!(
            find_cloudflare_deployment_check(checks).map(|check| check.id),
            Some(3)
        );
    }

    #[test]
    fn deployment_check_ignores_same_name_from_other_app() {
        let checks = vec![check_run(1, "Cloudflare Pages", "github-actions")];

        assert!(find_cloudflare_deployment_check(checks).is_none());
    }

    fn check_run(id: u64, name: &str, app: &str) -> CheckRun {
        CheckRun {
            id,
            name: name.to_owned(),
            status: "completed".to_owned(),
            conclusion: Some("success".to_owned()),
            details_url: None,
            app: CheckRunApp {
                slug: app.to_owned(),
            },
        }
    }
}
