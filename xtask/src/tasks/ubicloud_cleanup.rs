use std::env;

use crate::modules::cache_cleanup::{ActionsCacheEntry, CacheCleanupOptions, run_cache_cleanup};
use anyhow::{Result, bail};
use clap::Args;

use crate::modules::ubicloud::{GithubCacheEntry, Ubicloud};

const DEFAULT_API_TOKEN_ENV: &str = "UBICLOUD_API_TOKEN";

#[derive(Args)]
pub(crate) struct UbicloudCleanupArgs {
    #[arg(long = "project", value_name = "PROJECT")]
    pub(crate) project: String,

    #[arg(long = "installation", value_name = "INSTALLATION")]
    pub(crate) installation: String,

    #[arg(long = "repository", value_name = "REPOSITORY")]
    pub(crate) repository: String,

    #[arg(
        long = "api-token",
        value_name = "TOKEN",
        help = "Ubicloud API token. Falls back to UBICLOUD_API_TOKEN"
    )]
    pub(crate) api_token: Option<String>,

    #[command(flatten)]
    pub(crate) cleanup: CacheCleanupOptions,
}

pub(crate) fn run(args: UbicloudCleanupArgs) -> Result<()> {
    let UbicloudCleanupArgs {
        project,
        installation,
        repository,
        api_token,
        cleanup,
    } = args;

    let api_token = resolve_api_token(api_token.as_deref())?;
    let client = Ubicloud::new(api_token)?;
    let cache_entries = client
        .list_github_cache_entries(&project, &installation, &repository)?
        .items
        .into_iter()
        .map(to_actions_cache_entry)
        .collect();

    run_cache_cleanup(cleanup, cache_entries, |entry| {
        client.delete_github_cache_entry(&project, &installation, &repository, &entry.id)
    })
}

fn resolve_api_token(api_token: Option<&str>) -> Result<String> {
    if let Some(api_token) = api_token
        && !api_token.trim().is_empty()
    {
        return Ok(api_token.to_owned());
    }

    if let Ok(api_token) = env::var(DEFAULT_API_TOKEN_ENV)
        && !api_token.trim().is_empty()
    {
        return Ok(api_token);
    }

    bail!("Ubicloud API token is required. Pass --api-token or set {DEFAULT_API_TOKEN_ENV}");
}

fn to_actions_cache_entry(entry: GithubCacheEntry) -> ActionsCacheEntry {
    ActionsCacheEntry {
        id: entry.id,
        key: entry.key,
        size: entry.size,
        created_at: entry.created_at,
        last_accessed_at: entry.last_accessed_at,
    }
}
