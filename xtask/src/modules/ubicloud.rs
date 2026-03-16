use std::time::Duration;

use anyhow::{Context, Result, bail};
use reqwest::blocking::{Client, Response};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

const BASE_URL: &str = "https://api.ubicloud.com";
const HTTP_CONNECT_TIMEOUT_SECS: u64 = 10;
const HTTP_REQUEST_TIMEOUT_SECS: u64 = 30;

pub(crate) struct Ubicloud {
    client: Client,
    api_token: String,
}

impl Ubicloud {
    pub(crate) fn new(api_token: String) -> Result<Self> {
        let client = Client::builder()
            .connect_timeout(Duration::from_secs(HTTP_CONNECT_TIMEOUT_SECS))
            .timeout(Duration::from_secs(HTTP_REQUEST_TIMEOUT_SECS))
            .build()
            .context("failed to create Ubicloud HTTP client")?;

        Ok(Self { client, api_token })
    }

    pub(crate) fn list_github_cache_entries(
        &self,
        project: &str,
        installation: &str,
        repository: &str,
    ) -> Result<GithubCacheEntries> {
        let url = build_github_cache_entries_url(project, installation, repository);

        self.get_json(&url)
    }

    pub(crate) fn delete_github_cache_entry(
        &self,
        project: &str,
        installation: &str,
        repository: &str,
        cache_entry_id: &str,
    ) -> Result<()> {
        let url = build_github_cache_entry_url(project, installation, repository, cache_entry_id);

        self.delete(&url)
    }

    fn get_json<T>(&self, url: &str) -> Result<T>
    where
        T: DeserializeOwned,
    {
        let response = self
            .client
            .get(url)
            .bearer_auth(&self.api_token)
            .send()
            .with_context(|| format!("failed to send request to Ubicloud API: {url}"))?;

        Self::parse_json_response(response)
    }

    fn delete(&self, url: &str) -> Result<()> {
        let response = self
            .client
            .delete(url)
            .bearer_auth(&self.api_token)
            .send()
            .with_context(|| format!("failed to send request to Ubicloud API: {url}"))?;

        Self::ensure_success(response).map(|_| ())
    }

    fn parse_json_response<T>(response: Response) -> Result<T>
    where
        T: DeserializeOwned,
    {
        let response = Self::ensure_success(response)?;

        response
            .json()
            .context("failed to parse Ubicloud API response JSON")
    }

    fn ensure_success(response: Response) -> Result<Response> {
        let status = response.status();

        if !status.is_success() {
            let body = response
                .text()
                .context("failed to read Ubicloud API error response body")?;

            if let Ok(error) = serde_json::from_str::<UbicloudErrorResponse>(&body) {
                let error_type = error.error.type_;

                bail!(
                    "Ubicloud API request failed with status {status}: {} ({error_type})",
                    error.error.message
                );
            }

            if body.is_empty() {
                bail!("Ubicloud API request failed with status {status}");
            }

            bail!("Ubicloud API request failed with status {status}: {body}");
        }

        Ok(response)
    }
}

fn build_github_cache_entries_url(project: &str, installation: &str, repository: &str) -> String {
    format!(
        "{}/project/{}/github/{}/repository/{}/cache",
        BASE_URL,
        urlencoding::encode(project),
        urlencoding::encode(installation),
        urlencoding::encode(repository),
    )
}

fn build_github_cache_entry_url(
    project: &str,
    installation: &str,
    repository: &str,
    cache_entry_id: &str,
) -> String {
    format!(
        "{}/project/{}/github/{}/repository/{}/cache/{}",
        BASE_URL,
        urlencoding::encode(project),
        urlencoding::encode(installation),
        urlencoding::encode(repository),
        urlencoding::encode(cache_entry_id),
    )
}

#[derive(Debug, Deserialize)]
struct UbicloudErrorResponse {
    error: UbicloudError,
}

#[derive(Debug, Deserialize)]
struct UbicloudError {
    message: String,
    #[serde(rename = "type")]
    type_: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct GithubCacheEntries {
    pub(crate) count: usize,
    pub(crate) items: Vec<GithubCacheEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct GithubCacheEntry {
    pub(crate) installation_name: String,
    pub(crate) repository_name: String,
    pub(crate) id: String,
    pub(crate) key: String,
    pub(crate) size: u64,
}
