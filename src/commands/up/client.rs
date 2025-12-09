use anyhow::{Context, Result, bail};
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::blocking::Client;
use reqwest::header::USER_AGENT;
use serde::Deserialize;
use std::io::{Read, Write};
use std::path::PathBuf;

#[derive(Deserialize, Debug, Clone)]
pub struct Release {
    pub tag_name: String,
    pub assets: Vec<Asset>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Asset {
    pub name: String,
    pub browser_download_url: String,
    pub size: u64,

    #[cfg(test)]
    pub version: String,
    #[cfg(test)]
    pub content: Option<String>,
}

pub trait ReleaseClient {
    fn get_release(&self, version: Option<&str>, canary: bool) -> Result<Release>;
    fn download_asset(&self, asset: &Asset) -> Result<PathBuf>;
}

pub struct GitHubClient {
    client: Client,
}

impl GitHubClient {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }
}

impl ReleaseClient for GitHubClient {
    fn get_release(&self, version: Option<&str>, canary: bool) -> Result<Release> {
        let url = if let Some(v) = version {
            let tag = if v.starts_with('v') {
                v.to_string()
            } else {
                format!("v{}", v)
            };
            format!(
                "https://api.github.com/repos/i582/acton/releases/tags/{}",
                tag
            )
        } else if canary {
            "https://api.github.com/repos/i582/acton/releases/tags/canary".to_string()
        } else {
            "https://api.github.com/repos/i582/acton/releases/latest".to_string()
        };

        let resp = self
            .client
            .get(&url)
            .header(USER_AGENT, "acton-cli")
            .send()
            .context("Failed to fetch release info from GitHub")?;

        if !resp.status().is_success() {
            if resp.status().as_u16() == 404 {
                bail!("Release not found.");
            }
            bail!("GitHub API request failed: {}", resp.status());
        }

        let release: Release = resp.json().context("Failed to parse release JSON")?;
        Ok(release)
    }

    fn download_asset(&self, asset: &Asset) -> Result<PathBuf> {
        let pb = ProgressBar::new(asset.size);
        pb.set_style(ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")?
            .progress_chars("#>-"));

        let mut resp = self
            .client
            .get(&asset.browser_download_url)
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
        pb.finish_with_message("Download complete");

        let path = file.path().to_owned();
        file.keep()?;

        Ok(path)
    }
}
