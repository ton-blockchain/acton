use std::env;

use anyhow::Result;
use chrono::{DateTime, Duration, SecondsFormat, Utc};
use clap::Args;

const CI_ENV: &str = "CI";

pub(crate) const DEFAULT_LAST_ACCESSED_DELETE_AFTER_DAYS: i64 = 1;
pub(crate) const DEFAULT_CREATED_DELETE_AFTER_DAYS: i64 = 3;

#[derive(Args, Debug, Clone, Copy)]
pub(crate) struct CacheCleanupOptions {
    #[arg(
        long = "dry-run",
        num_args = 0..=1,
        default_missing_value = "true",
        value_name = "BOOL",
        help = "Show which cache entries would be deleted without deleting them. Defaults to `true` outside CI and `false` in CI when omitted"
    )]
    pub(crate) dry_run: Option<bool>,

    #[arg(
        long = "last-accessed-days",
        value_name = "DAYS",
        default_value_t = DEFAULT_LAST_ACCESSED_DELETE_AFTER_DAYS,
        value_parser = clap::value_parser!(i64).range(1..),
        help = "Delete entries if `last_accessed_at` is older than this many days"
    )]
    pub(crate) last_accessed_days: i64,

    #[arg(
        long = "created-days",
        value_name = "DAYS",
        default_value_t = DEFAULT_CREATED_DELETE_AFTER_DAYS,
        value_parser = clap::value_parser!(i64).range(1..),
        help = "Delete entries if `last_accessed_at` is missing and `created_at` is older than this many days"
    )]
    pub(crate) created_days: i64,
}

impl CacheCleanupOptions {
    pub(crate) const fn policy(self) -> CleanupPolicy {
        CleanupPolicy {
            last_accessed_days: self.last_accessed_days,
            created_days: self.created_days,
        }
    }
}

pub(crate) struct CleanupPolicy {
    pub(crate) last_accessed_days: i64,
    pub(crate) created_days: i64,
}

pub(crate) struct ActionsCacheEntry {
    pub(crate) id: String,
    pub(crate) key: String,
    pub(crate) size: u64,
    pub(crate) created_at: DateTime<Utc>,
    pub(crate) last_accessed_at: Option<DateTime<Utc>>,
}

pub(crate) fn run_cache_cleanup<F>(
    options: CacheCleanupOptions,
    entries: Vec<ActionsCacheEntry>,
    mut delete_entry: F,
) -> Result<()>
where
    F: FnMut(&ActionsCacheEntry) -> Result<()>,
{
    let policy = options.policy();
    let now = Utc::now();
    let (to_delete, to_keep) = plan_cache_cleanup(entries, &now, &policy);

    let is_ci = env::var(CI_ENV) == Ok("true".to_string());
    let dry_run = options.dry_run.unwrap_or(!is_ci);
    if options.dry_run.is_none() && !is_ci {
        println!(
            "Warning: `--dry-run` was not provided and `CI` is not `true`, so `dry-run=true` is used by default."
        );
        println!();
    }

    print_prune_plan(dry_run, &policy, &to_delete, &to_keep);

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
        delete_entry(entry)?;
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

fn print_prune_plan(
    dry_run: bool,
    policy: &CleanupPolicy,
    to_delete: &[ActionsCacheEntry],
    to_keep: &[ActionsCacheEntry],
) {
    println!("Prune plan for project");
    println!(
        "Delete cache entries with `last_accessed_at` older than {} day(s).",
        policy.last_accessed_days
    );
    println!(
        "If `last_accessed_at` is missing, delete cache entries with `created_at` older than {} day(s).",
        policy.created_days
    );
    println!(
        "If deleting would leave 0 cache entries with a real `last_accessed_at`, keep entries whose `last_accessed_at` is not `never`."
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

fn print_entries_table(title: &str, entries: &[ActionsCacheEntry]) {
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
        "{:<id_width$}  {:>size_width$}  {:<created_at_width$}  {:<last_accessed_at_width$}  Key",
        "ID", "Size", "Created At", "Last Accessed At",
    );

    for entry in entries {
        let created_at = format_timestamp(&entry.created_at);
        let last_accessed_at = format_optional_timestamp(entry.last_accessed_at.as_ref());
        println!(
            "{:<id_width$}  {:>size_width$}  {:<created_at_width$}  {:<last_accessed_at_width$}  {}",
            entry.id,
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

fn should_delete(entry: &ActionsCacheEntry, now: &DateTime<Utc>, policy: &CleanupPolicy) -> bool {
    let last_accessed_cutoff = now.to_owned() - Duration::days(policy.last_accessed_days);
    let created_cutoff = now.to_owned() - Duration::days(policy.created_days);

    match entry.last_accessed_at.as_ref() {
        Some(last_accessed_at) => last_accessed_at < &last_accessed_cutoff,
        None => entry.created_at < created_cutoff,
    }
}

fn plan_cache_cleanup(
    entries: Vec<ActionsCacheEntry>,
    now: &DateTime<Utc>,
    policy: &CleanupPolicy,
) -> (Vec<ActionsCacheEntry>, Vec<ActionsCacheEntry>) {
    let (to_delete, to_keep): (Vec<ActionsCacheEntry>, Vec<ActionsCacheEntry>) = entries
        .into_iter()
        .partition(|entry| should_delete(entry, now, policy));

    if to_keep.iter().any(|entry| entry.last_accessed_at.is_some()) {
        return (to_delete, to_keep);
    }

    let (protected_entries, to_delete): (Vec<ActionsCacheEntry>, Vec<ActionsCacheEntry>) =
        to_delete
            .into_iter()
            .partition(|entry| entry.last_accessed_at.is_some());

    let to_keep = to_keep.into_iter().chain(protected_entries).collect();

    (to_delete, to_keep)
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
