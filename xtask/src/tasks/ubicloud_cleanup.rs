use std::env;

use anyhow::{Result, bail};
use chrono::{DateTime, Duration, SecondsFormat, Utc};
use clap::Args;

use crate::modules::ubicloud::{GithubCacheEntry, Ubicloud};

const DEFAULT_API_TOKEN_ENV: &str = "UBICLOUD_API_TOKEN";
const LAST_ACCESSED_DELETE_AFTER_DAYS: i64 = 1;
const CREATED_DELETE_AFTER_DAYS: i64 = 3;

struct CleanupPolicy {
    last_accessed_days: i64,
    created_days: i64,
}

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

    #[arg(
        long = "dry-run",
        help = "Show which cache entries would be deleted without deleting them"
    )]
    pub(crate) dry_run: bool,

    #[arg(
        long = "last-accessed-days",
        value_name = "DAYS",
        default_value_t = LAST_ACCESSED_DELETE_AFTER_DAYS,
        value_parser = clap::value_parser!(i64).range(1..),
        help = "Delete entries if `last_accessed_at` is older than this many days"
    )]
    pub(crate) last_accessed_days: i64,

    #[arg(
        long = "created-days",
        value_name = "DAYS",
        default_value_t = CREATED_DELETE_AFTER_DAYS,
        value_parser = clap::value_parser!(i64).range(1..),
        help = "Delete entries if `last_accessed_at` is missing and `created_at` is older than this many days"
    )]
    pub(crate) created_days: i64,
}

pub(crate) fn run(args: UbicloudCleanupArgs) -> Result<()> {
    let UbicloudCleanupArgs {
        project,
        installation,
        repository,
        api_token,
        dry_run,
        last_accessed_days,
        created_days,
    } = args;

    let api_token = resolve_api_token(api_token.as_deref())?;
    let client = Ubicloud::new(api_token)?;
    let policy = CleanupPolicy {
        last_accessed_days,
        created_days,
    };
    let cache_entries = client.list_github_cache_entries(&project, &installation, &repository)?;
    let now = Utc::now();
    let (to_delete, to_keep): (Vec<GithubCacheEntry>, Vec<GithubCacheEntry>) = cache_entries
        .items
        .into_iter()
        .partition(|entry| should_delete(entry, &now, &policy));

    print_prune_plan(
        &project,
        &installation,
        &repository,
        dry_run,
        &policy,
        &to_delete,
        &to_keep,
    );

    if dry_run {
        println!();
        println!("Dry run: no cache entries were deleted.");
        return Ok(());
    }

    if to_delete.is_empty() {
        println!();
        println!("No cache entries to delete.");
        return Ok(());
    }

    println!();
    println!("Deleting {} cache entries...", to_delete.len());

    for entry in &to_delete {
        client.delete_github_cache_entry(&project, &installation, &repository, &entry.id)?;
        println!("Deleted {}  {}", entry.id, entry.key);
    }

    let deleted_size = to_delete.iter().map(|entry| entry.size).sum::<u64>();

    println!();
    println!(
        "Deleted {} cache entries, freed {}, kept {}.",
        to_delete.len(),
        human_size(deleted_size),
        to_keep.len()
    );

    Ok(())
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

fn print_prune_plan(
    project: &str,
    installation: &str,
    repository: &str,
    dry_run: bool,
    policy: &CleanupPolicy,
    to_delete: &[GithubCacheEntry],
    to_keep: &[GithubCacheEntry],
) {
    println!("Prune plan for {installation}/{repository} in project `{project}`");
    println!(
        "Delete cache entries with `last_accessed_at` older than {} day(s).",
        policy.last_accessed_days
    );
    println!(
        "If `last_accessed_at` is missing, delete cache entries with `created_at` older than {} day(s).",
        policy.created_days
    );

    println!();
    print_entries_table("Cache entries to keep", to_keep);

    println!();
    print_entries_table(
        if dry_run {
            "Cache entries that would be deleted"
        } else {
            "Cache entries to delete"
        },
        to_delete,
    );

    let delete_size = to_delete.iter().map(|entry| entry.size).sum::<u64>();
    let keep_size = to_keep.iter().map(|entry| entry.size).sum::<u64>();

    println!();
    println!(
        "Summary: keep {} cache entries ({}), delete {} cache entries ({}).",
        to_keep.len(),
        human_size(keep_size),
        to_delete.len(),
        human_size(delete_size)
    );
}

fn print_entries_table(title: &str, entries: &[GithubCacheEntry]) {
    println!("{title}");

    if entries.is_empty() {
        println!("No cache entries found.");
        return;
    }

    let id_width = entries
        .iter()
        .map(|entry| entry.id.len())
        .max()
        .unwrap_or(2)
        .max("ID".len());
    let installation_width = entries
        .iter()
        .map(|entry| entry.installation_name.len())
        .max()
        .unwrap_or(12)
        .max("Installation".len());
    let repository_width = entries
        .iter()
        .map(|entry| entry.repository_name.len())
        .max()
        .unwrap_or(10)
        .max("Repository".len());
    let size_width = entries
        .iter()
        .map(|entry| human_size(entry.size).len())
        .max()
        .unwrap_or(4)
        .max("Size".len());
    let created_at_width = entries
        .iter()
        .map(|entry| format_timestamp(&entry.created_at))
        .map(|value| value.len())
        .max()
        .unwrap_or(10)
        .max("Created At".len());
    let last_accessed_at_width = entries
        .iter()
        .map(|entry| format_optional_timestamp(entry.last_accessed_at.as_ref()))
        .map(|value| value.len())
        .max()
        .unwrap_or(16)
        .max("Last Accessed At".len());

    println!(
        "{:<id_width$}  {:<installation_width$}  {:<repository_width$}  {:>size_width$}  {:<created_at_width$}  {:<last_accessed_at_width$}  Key",
        "ID", "Installation", "Repository", "Size", "Created At", "Last Accessed At",
    );

    for entry in entries {
        let created_at = format_timestamp(&entry.created_at);
        let last_accessed_at = format_optional_timestamp(entry.last_accessed_at.as_ref());
        println!(
            "{:<id_width$}  {:<installation_width$}  {:<repository_width$}  {:>size_width$}  {:<created_at_width$}  {:<last_accessed_at_width$}  {}",
            entry.id,
            entry.installation_name,
            entry.repository_name,
            human_size(entry.size),
            created_at,
            last_accessed_at,
            entry.key,
        );
    }
}

fn format_timestamp(timestamp: &DateTime<Utc>) -> String {
    timestamp.to_rfc3339_opts(SecondsFormat::Secs, true)
}

fn format_optional_timestamp(timestamp: Option<&DateTime<Utc>>) -> String {
    timestamp
        .map(format_timestamp)
        .unwrap_or_else(|| "never".to_owned())
}

fn should_delete(entry: &GithubCacheEntry, now: &DateTime<Utc>, policy: &CleanupPolicy) -> bool {
    let last_accessed_cutoff = now.to_owned() - Duration::days(policy.last_accessed_days);
    let created_cutoff = now.to_owned() - Duration::days(policy.created_days);

    match entry.last_accessed_at.as_ref() {
        Some(last_accessed_at) => last_accessed_at < &last_accessed_cutoff,
        None => entry.created_at < created_cutoff,
    }
}

fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];

    if bytes < 1024 {
        return format!("{bytes} B");
    }

    let mut unit_index = 0usize;
    let mut value = bytes as f64;

    while value >= 1024.0 && unit_index + 1 < UNITS.len() {
        value /= 1024.0;
        unit_index += 1;
    }

    format!("{value:.1} {}", UNITS[unit_index])
}
