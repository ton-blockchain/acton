use acton_config::color::OwoColorize;
use anyhow::{Context, Result, bail};
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::blocking::Client;
use reqwest::header::USER_AGENT;
use serde::Deserialize;
use std::io::{Read, Write};
use std::path::PathBuf;

const GITHUB_RELEASE_REPOSITORIES: [&str; 2] = ["ton-blockchain/acton", "i582/acton-public"];

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
    NotFound,
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
            Err(_) => return RepoFetchResult::NotFound,
        };

        if resp.status().as_u16() != 200 {
            return RepoFetchResult::NotFound;
        }

        if !resp.status().is_success() {
            return RepoFetchResult::NotFound;
        }

        match resp.json() {
            Ok(release) => RepoFetchResult::Found(release),
            Err(_) => RepoFetchResult::NotFound,
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
                Err(_) => return RepoFetchResult::NotFound,
            };

            if resp.status().as_u16() == 404 {
                return RepoFetchResult::NotFound;
            }

            if !resp.status().is_success() {
                return RepoFetchResult::NotFound;
            }

            let releases: Vec<Release> = match resp.json() {
                Ok(releases) => releases,
                Err(_) => {
                    return RepoFetchResult::NotFound;
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

        for repo in GITHUB_RELEASE_REPOSITORIES {
            match self.fetch_release_from_repo(repo, &path) {
                RepoFetchResult::Found(release) => return Ok(release),
                RepoFetchResult::NotFound => continue,
            }
        }

        bail!("Failed to fetch release info from GitHub");
    }

    fn list_releases(&self) -> Result<Vec<String>> {
        let mut tags = Vec::new();
        let mut success = false;

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
                RepoFetchResult::NotFound => {}
            }
        }

        if success {
            return Ok(tags);
        }

        bail!("Failed to fetch releases from GitHub")
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
            .context("Failed to download asset")?;

        if !resp.status().is_success() {
            bail!("Failed to download asset: {}", resp.status());
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
