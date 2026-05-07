use acton_config::color::OwoColorize;
use anyhow::{Context, Result, bail};
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::blocking::Client;
use reqwest::header::USER_AGENT;
use serde::Deserialize;
#[cfg(debug_assertions)]
use std::env;
use std::io::{Read, Write};
use std::path::PathBuf;

const GITHUB_RELEASE_REPOSITORY: &str = "ton-blockchain/acton";
#[cfg(debug_assertions)]
const TEST_GITHUB_API_BASE_ENV: &str = "ACTON_TEST_UP_GITHUB_API_BASE"; // non-release test hook only

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
    #[cfg(test)]
    pub raw_bytes: Option<Vec<u8>>,
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

impl GitHubClient {
    pub(super) fn new(token: Option<String>) -> Self {
        Self {
            client: crate::http::blocking_client_builder()
                .build()
                .expect("failed to build GitHub HTTP client"),
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

    fn api_base() -> String {
        if let Some(base) = test_github_api_base_override() {
            let base = base.trim().trim_end_matches('/');
            if !base.is_empty() {
                return format!("{base}/repos/{GITHUB_RELEASE_REPOSITORY}");
            }
        }

        format!("https://api.github.com/repos/{GITHUB_RELEASE_REPOSITORY}")
    }

    fn request(&self, url: &str) -> reqwest::blocking::RequestBuilder {
        let mut req = self
            .client
            .get(url)
            .header(USER_AGENT, crate::build_info::user_agent());
        if let Some(token) = &self.token {
            req = req.header("Authorization", format!("token {token}"));
        }
        req
    }

    fn fetch_release(&self, path: &str) -> Result<Option<Release>> {
        let url = format!("{}/{path}", Self::api_base());
        let resp = match self.request(&url).send() {
            Ok(resp) => resp,
            Err(err) => bail!("could not reach GitHub release API: {err}"),
        };

        if resp.status().as_u16() == 404 {
            return Ok(None);
        }

        if !resp.status().is_success() {
            bail!("GitHub release API returned {}", resp.status());
        }

        resp.json()
            .context("GitHub release API returned invalid JSON")
            .map(Some)
    }

    fn fetch_release_tags(&self) -> Result<Vec<String>> {
        let mut tags = Vec::new();
        let per_page = 100;
        let mut page = 1;

        loop {
            let url = format!(
                "{}/releases?per_page={per_page}&page={page}",
                Self::api_base()
            );

            let resp = match self.request(&url).send() {
                Ok(resp) => resp,
                Err(err) => bail!("could not reach GitHub release API: {err}"),
            };

            if !resp.status().is_success() {
                bail!(
                    "GitHub release API returned {} while listing releases",
                    resp.status()
                );
            }

            let releases: Vec<Release> = match resp.json() {
                Ok(releases) => releases,
                Err(_) => {
                    bail!("GitHub release API returned invalid JSON while listing releases");
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

        Ok(tags)
    }
}

#[cfg(debug_assertions)]
fn test_github_api_base_override() -> Option<String> {
    env::var(TEST_GITHUB_API_BASE_ENV).ok()
}

#[cfg(not(debug_assertions))]
fn test_github_api_base_override() -> Option<String> {
    None
}

impl ReleaseClient for GitHubClient {
    fn get_release(&self, version: Option<&str>, trunk: bool) -> Result<Release> {
        let path = Self::release_request_path(version, trunk);
        let requested = requested_release_label(version, trunk);

        match self.fetch_release(&path) {
            Ok(Some(release)) => Ok(release),
            Ok(None) => bail!("Release not found: {requested}"),
            Err(err) if is_release_api_connectivity_error(&err) => {
                bail!(
                    "Failed to look up {requested} on GitHub. Check your network connection and try again."
                );
            }
            Err(err) => bail!("Failed to look up {requested} on GitHub. {err}"),
        }
    }

    fn list_releases(&self) -> Result<Vec<String>> {
        match self.fetch_release_tags() {
            Ok(tags) => Ok(tags),
            Err(err) if is_release_api_connectivity_error(&err) => {
                bail!(
                    "Failed to fetch the release list from GitHub. Check your network connection and try again."
                );
            }
            Err(err) => bail!("Failed to fetch the release list from GitHub. {err}"),
        }
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

        let mut resp = match req
            .header(USER_AGENT, crate::build_info::user_agent())
            .send()
        {
            Ok(resp) => resp,
            Err(err) if err.is_connect() || err.is_timeout() => {
                bail!(
                    "Failed to download {} from GitHub. Check your network connection and try again.",
                    asset.name
                );
            }
            Err(err) => {
                return Err(err)
                    .with_context(|| format!("Failed to download {} from GitHub", asset.name));
            }
        };

        if !resp.status().is_success() {
            bail!(
                "GitHub returned {} while downloading {}",
                resp.status(),
                asset.name
            );
        }

        let mut file = tempfile::NamedTempFile::new()
            .with_context(|| format!("Failed to create a temporary file for {}", asset.name))?;
        let mut buf = [0; 8192];
        let mut downloaded = 0;

        loop {
            let n = resp.read(&mut buf).with_context(|| {
                format!(
                    "Failed while downloading {} from GitHub. Check your network connection and try again.",
                    asset.name
                )
            })?;
            if n == 0 {
                break;
            }
            file.write_all(&buf[..n])
                .with_context(|| format!("Failed to write {} to a temporary file", asset.name))?;
            downloaded += n as u64;
            pb.set_position(downloaded);
        }
        pb.finish_and_clear();

        let path = file.path().to_owned();
        file.keep()
            .with_context(|| format!("Failed to persist the downloaded file for {}", asset.name))?;

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

fn is_release_api_connectivity_error(error: &anyhow::Error) -> bool {
    error
        .to_string()
        .starts_with("could not reach GitHub release API")
}
