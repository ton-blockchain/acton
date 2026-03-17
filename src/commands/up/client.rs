use acton_config::color::OwoColorize;
use anyhow::{Context, Result, bail};
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::blocking::Client;
use reqwest::header::USER_AGENT;
use serde::Deserialize;
use std::io::{Read, Write};
use std::path::PathBuf;

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

impl GitHubClient {
    pub(super) fn new(token: Option<String>) -> Self {
        Self {
            client: Client::new(),
            token,
        }
    }
}

impl ReleaseClient for GitHubClient {
    fn get_release(&self, version: Option<&str>, trunk: bool) -> Result<Release> {
        let url = if let Some(v) = version {
            let tag = if v.starts_with('v') {
                v.to_string()
            } else {
                format!("v{v}")
            };
            format!("https://api.github.com/repos/i582/acton/releases/tags/{tag}")
        } else if trunk {
            "https://api.github.com/repos/i582/acton/releases/tags/trunk".to_string()
        } else {
            "https://api.github.com/repos/i582/acton/releases/latest".to_string()
        };

        let mut req = self.client.get(&url).header(USER_AGENT, "acton-cli");

        if let Some(token) = &self.token {
            req = req.header("Authorization", format!("token {token}"));
        }

        let resp = req
            .send()
            .context("Failed to fetch release info from GitHub")?;

        if !resp.status().is_success() {
            if resp.status().as_u16() == 404 {
                if let Some(v) = version {
                    bail!("Release not found: {v}");
                }
                bail!("Release not found");
            }
            bail!("GitHub API request failed: {}", resp.status());
        }

        let release: Release = resp.json().context("Failed to parse release JSON")?;
        Ok(release)
    }

    fn list_releases(&self) -> Result<Vec<String>> {
        let url = "https://api.github.com/repos/i582/acton/releases";

        let mut req = self.client.get(url).header(USER_AGENT, "acton-cli");

        if let Some(token) = &self.token {
            req = req.header("Authorization", format!("token {token}"));
        }

        let resp = req.send().context("Failed to fetch releases from GitHub")?;

        if !resp.status().is_success() {
            bail!("GitHub API request failed: {}", resp.status());
        }

        let releases: Vec<Release> = resp.json().context("Failed to parse releases JSON")?;
        Ok(releases.into_iter().map(|r| r.tag_name).collect())
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
