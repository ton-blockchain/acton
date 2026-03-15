use std::env;

use anyhow::{Result, bail};
use clap::Args;

use crate::modules::ubicloud::Ubicloud;

const DEFAULT_API_TOKEN_ENV: &str = "UBICLOUD_API_TOKEN";
const DEFAULT_KEEP_COUNT: usize = 5;

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
        long = "keep",
        value_name = "COUNT",
        help = "Keep the last N cache entries from the current Ubicloud API response order. Defaults to 5; omitting --keep enables dry-run automatically"
    )]
    pub(crate) keep: Option<usize>,

    #[arg(
        long = "dry-run",
        help = "Show which cache entries would be deleted without deleting them"
    )]
    pub(crate) dry_run: bool,
}

pub(crate) fn run(args: UbicloudCleanupArgs) -> Result<()> {
    let UbicloudCleanupArgs {
        project,
        installation,
        repository,
        api_token,
        keep,
        dry_run,
    } = args;

    let api_token = resolve_api_token(api_token.as_deref())?;
    let client = Ubicloud::new(api_token)?;
    let cache_entries = client.list_github_cache_entries(&project, &installation, &repository)?;
    let keep_was_explicit = keep.is_some();
    let keep = keep.unwrap_or(DEFAULT_KEEP_COUNT);
    let dry_run = dry_run || !keep_was_explicit;

    let split_index = cache_entries.items.len().saturating_sub(keep);
    let (to_delete, to_keep) = cache_entries.items.split_at(split_index);

    print_prune_plan(
        &project,
        &installation,
        &repository,
        keep,
        dry_run,
        to_delete,
        to_keep,
    );

    if dry_run {
        println!();
        if !keep_was_explicit {
            println!("Safety mode: --keep was not provided, so dry-run was enabled automatically.");
        }
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

    for entry in to_delete {
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

    bail!(
        "Ubicloud API token is required. Pass --api-token or set {}",
        DEFAULT_API_TOKEN_ENV
    );
}

fn print_prune_plan(
    project: &str,
    installation: &str,
    repository: &str,
    keep: usize,
    dry_run: bool,
    to_delete: &[crate::modules::ubicloud::GithubCacheEntry],
    to_keep: &[crate::modules::ubicloud::GithubCacheEntry],
) {
    println!(
        "Prune plan for {}/{} in project `{}`",
        installation, repository, project
    );
    println!(
        "Keeping the last {} cache entries from the current Ubicloud API response order.",
        keep
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

fn print_entries_table(title: &str, entries: &[crate::modules::ubicloud::GithubCacheEntry]) {
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

    println!(
        "{:<id_width$}  {:<installation_width$}  {:<repository_width$}  {:>size_width$}  Key",
        "ID", "Installation", "Repository", "Size",
    );

    for entry in entries {
        println!(
            "{:<id_width$}  {:<installation_width$}  {:<repository_width$}  {:>size_width$}  {}",
            entry.id,
            entry.installation_name,
            entry.repository_name,
            human_size(entry.size),
            entry.key,
        );
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
