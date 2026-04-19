use acton_config::color::OwoColorize;
use anyhow::{Context, Result, bail};
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::blocking::Client;
use reqwest::header::USER_AGENT;
use serde::Deserialize;
use std::env;
use std::io::{Read, Write};
use std::path::PathBuf;

const GITHUB_RELEASE_REPOSITORIES: [&str; 2] = ["ton-blockchain/acton", "i582/acton-public"];
const TEST_GITHUB_API_BASE_ENV: &str = "ACTON_TEST_UP_GITHUB_API_BASE"; // integration tests only

#[derive(Deserialize, Debug, Clone)]
pub(super) struct Release {
    pub tag_name: String,
    pub assets: Vec<Asset>,
}

#[derive(Deserialize, Debug, Clone)]
pub(super) struct Asset {
    pub name: String,
    pub url: String,
    pub browser_download_url: String,
    pub size: u64,

    #[cfg(test)]
    pub version: String,
    #[cfg(test)]
    pub content: Option<String>,
}

pub(super) trait ReleaseClient {
    fn get_release(&self, version: Option<&str>, trunk: bool) -> Result<Release>;
    fn list_releases(&self) -> Result<Vec<String>>;
    fn download_asset(&self, asset: &Asset) -> Result<PathBuf>;
}

pub(super) struct GitHubClient {
    client: Client,
    token: Option<String>,
}

enum RepoFetchResult<T> {
    Found(T),
    Missing,
    Failed(String),
}

impl GitHubClient {
    pub(super) fn new(token: Option<String>) -> Self {
        Self {
            client: Client::new(),
            token,
        }
    }

    fn release_request_path(version: Option<&str>, trunk: bool) -> String {
        if let Some(v) = version {
            let normalized = v.trim();
            if normalized.eq_ignore_ascii_case("trunk") || normalized.eq_ignore_ascii_case("vtrunk")
            {
                "releases/tags/trunk".to_string()
            } else {
                let tag = if normalized.starts_with('v') {
                    normalized.to_string()
                } else {
                    format!("v{normalized}")
                };
                format!("releases/tags/{tag}")
            }
        } else if trunk {
            "releases/tags/trunk".to_string()
        } else {
            "releases/latest".to_string()
        }
    }

    fn api_base_for_repo(repo: &str) -> String {
        if let Ok(base) = env::var(TEST_GITHUB_API_BASE_ENV) {
            let base = base.trim().trim_end_matches('/');
            if !base.is_empty() {
                return format!("{base}/repos/{repo}");
            }
        }

        format!("https://api.github.com/repos/{repo}")
    }

    fn request(&self, url: &str) -> reqwest::blocking::RequestBuilder {
        let mut req = self.client.get(url).header(USER_AGENT, "acton-cli");
        if let Some(token) = &self.token {
            req = req.header("Authorization", format!("token {token}"));
        }
        req
    }

    fn fetch_release_from_repo(&self, repo: &str, path: &str) -> RepoFetchResult<Release> {
        let url = format!("{}/{path}", Self::api_base_for_repo(repo));
        let resp = match self.request(&url).send() {
            Ok(resp) => resp,
            Err(err) => {
                return RepoFetchResult::Failed(format!(
                    "could not reach GitHub release API for {repo}: {err}"
                ));
            }
        };

        if resp.status().as_u16() == 404 {
            return RepoFetchResult::Missing;
        }

        if !resp.status().is_success() {
            return RepoFetchResult::Failed(format!(
                "GitHub release API returned {} for {repo}",
                resp.status()
            ));
        }

        match resp.json() {
            Ok(release) => RepoFetchResult::Found(release),
            Err(err) => RepoFetchResult::Failed(format!(
                "GitHub release API returned invalid JSON for {repo}: {err}"
            )),
        }
    }

    fn fetch_release_tags_from_repo(&self, repo: &str) -> RepoFetchResult<Vec<String>> {
        let mut tags = Vec::new();
        let per_page = 100;
        let mut page = 1;

        loop {
            let url = format!(
                "{}/releases?per_page={per_page}&page={page}",
                Self::api_base_for_repo(repo)
            );

            let resp = match self.request(&url).send() {
                Ok(resp) => resp,
                Err(err) => {
                    return RepoFetchResult::Failed(format!(
                        "could not reach GitHub release API for {repo}: {err}"
                    ));
                }
            };

            if resp.status().as_u16() == 404 {
                return RepoFetchResult::Missing;
            }

            if !resp.status().is_success() {
                return RepoFetchResult::Failed(format!(
                    "GitHub release API returned {} while listing releases for {repo}",
                    resp.status()
                ));
            }

            let releases: Vec<Release> = match resp.json() {
                Ok(releases) => releases,
                Err(_) => {
                    return RepoFetchResult::Failed(format!(
                        "GitHub release API returned invalid JSON while listing releases for {repo}"
                    ));
                }
            };
            let page_len = releases.len();

            if releases.is_empty() {
                break;
            }

            for release in releases {
                if !tags.iter().any(|tag| tag == &release.tag_name) {
                    tags.push(release.tag_name);
                }
            }

            if page_len < per_page {
                break;
            }

            page += 1;
        }

        RepoFetchResult::Found(tags)
    }
}

impl ReleaseClient for GitHubClient {
    fn get_release(&self, version: Option<&str>, trunk: bool) -> Result<Release> {
        let path = Self::release_request_path(version, trunk);
        let mut errors = Vec::new();

        for repo in GITHUB_RELEASE_REPOSITORIES {
            match self.fetch_release_from_repo(repo, &path) {
                RepoFetchResult::Found(release) => return Ok(release),
                RepoFetchResult::Missing => continue,
                RepoFetchResult::Failed(err) => errors.push(err),
            }
        }

        if errors.is_empty() {
            let requested = requested_release_label(version, trunk);
            bail!("Release not found: {requested}");
        }

        let requested = requested_release_label(version, trunk);
        if all_release_errors_are_network_related(&errors) {
            bail!(
                "Failed to look up {} on GitHub. Check your network connection and try again.",
                requested
            );
        }
        bail!(
            "Failed to look up {requested} on GitHub. {}",
            errors.join("; ")
        );
    }

    fn list_releases(&self) -> Result<Vec<String>> {
        let mut tags = Vec::new();
        let mut success = false;
        let mut errors = Vec::new();

        for repo in GITHUB_RELEASE_REPOSITORIES {
            match self.fetch_release_tags_from_repo(repo) {
                RepoFetchResult::Found(repo_tags) => {
                    success = true;
                    for tag in repo_tags {
                        if !tags.iter().any(|existing| existing == &tag) {
                            tags.push(tag);
                        }
                    }
                }
                RepoFetchResult::Missing => {}
                RepoFetchResult::Failed(err) => errors.push(err),
            }
        }

        if success {
            return Ok(tags);
        }

        if errors.is_empty() {
            bail!("No GitHub release metadata was found in the configured repositories");
        }

        if all_release_errors_are_network_related(&errors) {
            bail!(
                "Failed to fetch the release list from GitHub. Check your network connection and try again."
            );
        }

        bail!(
            "Failed to fetch the release list from GitHub. {}",
            errors.join("; ")
        )
    }

    fn download_asset(&self, asset: &Asset) -> Result<PathBuf> {
        println!("       {} {}", "Found".green().bold(), asset.name);

        let pb = ProgressBar::new(asset.size);
        pb.set_style(
            ProgressStyle::default_bar()
                .template(&format!(
                    " {} [{{bar:40.}}] {{bytes}}/{{total_bytes}} ({{eta}})",
                    "Downloading".green().bold()
                ))?
                .progress_chars("=> "),
        );

        let mut req = self.client.get(&asset.browser_download_url);

        if let Some(token) = &self.token {
            req = self
                .client
                .get(&asset.url)
                .header("Accept", "application/octet-stream")
                .header("Authorization", format!("token {token}"));
        }

        let mut resp = req
            .header(USER_AGENT, "acton-cli")
            .send()
            .with_context(|| format!("Failed to download {} from GitHub", asset.name))?;

        if !resp.status().is_success() {
            bail!(
                "GitHub returned {} while downloading {}",
                resp.status(),
                asset.name
            );
        }

        let mut file = tempfile::NamedTempFile::new()?;
        let mut buf = [0; 8192];
        let mut downloaded = 0;

        loop {
            let n = resp.read(&mut buf)?;
            if n == 0 {
                break;
            }
            file.write_all(&buf[..n])?;
            downloaded += n as u64;
            pb.set_position(downloaded);
        }
        pb.finish_and_clear();

        let path = file.path().to_owned();
        file.keep()?;

        Ok(path)
    }
}

fn requested_release_label(version: Option<&str>, trunk: bool) -> String {
    if let Some(version) = version {
        return format!("release `{}`", version.trim());
    }

    if trunk {
        return "the `trunk` release".to_owned();
    }

    "the latest release".to_owned()
}

fn all_release_errors_are_network_related(errors: &[String]) -> bool {
    !errors.is_empty()
        && errors
            .iter()
            .all(|error| error.starts_with("could not reach GitHub release API"))
}
