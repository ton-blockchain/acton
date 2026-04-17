use crate::modules::cache_cleanup::ActionsCacheEntry;
use crate::modules::cache_cleanup::{CacheCleanupOptions, run_cache_cleanup};
use crate::modules::github::{Github, GithubCacheEntry};
use anyhow::Result;
use clap::Args;

#[derive(Args)]
pub(crate) struct GithubCleanupArgs {
    #[command(flatten)]
    pub(crate) cleanup: CacheCleanupOptions,
}

pub(crate) fn run(args: GithubCleanupArgs) -> Result<()> {
    let GithubCleanupArgs { cleanup } = args;

    let github = Github::new();
    let cache_entries = github
        .list_cache_entries()?
        .into_iter()
        .map(to_actions_cache_entry)
        .collect();

    run_cache_cleanup(cleanup, cache_entries, |entry| {
        github.delete_cache_entry(&entry.id)
    })
}

fn to_actions_cache_entry(entry: GithubCacheEntry) -> ActionsCacheEntry {
    ActionsCacheEntry {
        id: entry.id.to_string(),
        branch: entry.branch,
        key: entry.key,
        size: entry.size_in_bytes,
        created_at: entry.created_at,
        last_accessed_at: entry.last_accessed_at,
    }
}
