use crate::commands::verify::new_verifier_backend;
use crate::http::blocking_client_builder;
use crate::paths::build_cache_dir;
use acton_config::config::project_root;
use anyhow::{Context, anyhow};
use log::debug;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tolk_compiler::abi::ContractABI;

const VERIFIER_ABI_CACHE_SUBDIR: &str = "verifier-abi";
const VERIFIER_ABI_CACHE_SCHEMA_VERSION: u32 = 1;
const VERIFIER_ABI_CACHE_TTL: Duration = Duration::from_secs(24 * 60 * 60);
const VERIFIER_REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Deserialize)]
struct VerifierAbiResponse {
    items: Vec<VerifierAbiItem>,
}

#[derive(Debug, Deserialize)]
struct VerifierAbiItem {
    code_hash: String,
    abi: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
struct VerifierAbiCacheEntry {
    schema_version: u32,
    code_hash: String,
    fetched_at: u64,
    abi: ContractABI,
}

pub(super) fn find_abi(code_hash: &str) -> anyhow::Result<Option<Arc<ContractABI>>> {
    let Some(code_hash) = normalize_code_hash(code_hash) else {
        return Ok(None);
    };

    let cache_path = cache_file_path(project_root(), &code_hash);
    let now = unix_timestamp();
    let cached = read_cache_entry(&cache_path, &code_hash);
    if cached
        .as_ref()
        .is_some_and(|entry| cache_entry_is_fresh(entry, now))
    {
        return Ok(cached.map(|entry| Arc::new(entry.abi)));
    }

    match fetch_abi(&code_hash) {
        Ok(Some(abi)) => {
            let entry = VerifierAbiCacheEntry {
                schema_version: VERIFIER_ABI_CACHE_SCHEMA_VERSION,
                code_hash,
                fetched_at: now,
                abi,
            };
            if let Err(err) = write_cache_entry(&cache_path, &entry) {
                debug!(
                    "Failed to write verifier ABI cache {}: {err:#}",
                    cache_path.display()
                );
            }
            Ok(Some(Arc::new(entry.abi)))
        }
        Ok(None) => Ok(None),
        Err(err) => {
            if let Some(entry) = cached {
                debug!(
                    "Failed to refresh verifier ABI for {}: {err:#}; using stale cache",
                    entry.code_hash
                );
                return Ok(Some(Arc::new(entry.abi)));
            }
            Err(err)
        }
    }
}

fn fetch_abi(code_hash: &str) -> anyhow::Result<Option<ContractABI>> {
    let backend = new_verifier_backend();
    let abi_endpoint = format!("{backend}/api/v1/abi");
    let url = format!("{abi_endpoint}?code_hash={code_hash}");

    let response = blocking_client_builder()
        .timeout(VERIFIER_REQUEST_TIMEOUT)
        .build()
        .context("Failed to build verifier HTTP client")?
        .get(url)
        .send()
        .context("Failed to fetch ABI from verifier")?
        .error_for_status()
        .context("Verifier returned an error while fetching ABI")?;
    let payload = response
        .json::<VerifierAbiResponse>()
        .context("Failed to parse verifier ABI response")?;

    let Some(item) = payload
        .items
        .into_iter()
        .find(|item| normalize_code_hash(&item.code_hash).as_deref() == Some(code_hash))
    else {
        return Ok(None);
    };

    serde_json::from_value(item.abi)
        .map(Some)
        .map_err(|err| anyhow!("Verifier returned an invalid compiler ABI: {err}"))
}

fn cache_file_path(root: &Path, code_hash: &str) -> PathBuf {
    build_cache_dir(root)
        .join(VERIFIER_ABI_CACHE_SUBDIR)
        .join(format!("{code_hash}.json"))
}

fn read_cache_entry(path: &Path, code_hash: &str) -> Option<VerifierAbiCacheEntry> {
    let entry = serde_json::from_slice::<VerifierAbiCacheEntry>(&fs::read(path).ok()?).ok()?;
    (entry.schema_version == VERIFIER_ABI_CACHE_SCHEMA_VERSION && entry.code_hash == code_hash)
        .then_some(entry)
}

fn write_cache_entry(path: &Path, entry: &VerifierAbiCacheEntry) -> anyhow::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow!("Verifier ABI cache path has no parent"))?;
    fs::create_dir_all(parent)?;
    fs::write(path, serde_json::to_vec_pretty(entry)?)?;
    Ok(())
}

fn cache_entry_is_fresh(entry: &VerifierAbiCacheEntry, now: u64) -> bool {
    entry
        .fetched_at
        .checked_add(VERIFIER_ABI_CACHE_TTL.as_secs())
        .is_some_and(|expires_at| now < expires_at)
}

fn unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn normalize_code_hash(code_hash: &str) -> Option<String> {
    let code_hash = code_hash.trim();
    let code_hash = code_hash
        .strip_prefix("0x")
        .or_else(|| code_hash.strip_prefix("0X"))
        .unwrap_or(code_hash);
    if code_hash.len() != 64 || !code_hash.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    Some(code_hash.to_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_verifier_code_hashes() {
        let expected = "ab".repeat(32);
        assert_eq!(
            normalize_code_hash(&format!("  0x{}  ", "AB".repeat(32))),
            Some(expected)
        );
        assert_eq!(normalize_code_hash("not-a-hash"), None);
    }

    #[test]
    fn cache_entry_expires_after_ttl() {
        let entry = VerifierAbiCacheEntry {
            schema_version: VERIFIER_ABI_CACHE_SCHEMA_VERSION,
            code_hash: "a".repeat(64),
            fetched_at: 100,
            abi: ContractABI::default(),
        };

        assert!(cache_entry_is_fresh(
            &entry,
            100 + VERIFIER_ABI_CACHE_TTL.as_secs() - 1
        ));
        assert!(!cache_entry_is_fresh(
            &entry,
            100 + VERIFIER_ABI_CACHE_TTL.as_secs()
        ));
    }

    #[test]
    fn cache_path_is_under_project_build_cache() {
        let path = cache_file_path(Path::new("/tmp/acton-project"), &"a".repeat(64));
        assert_eq!(
            path,
            PathBuf::from(
                "/tmp/acton-project/build/cache/verifier-abi/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa.json"
            )
        );
    }

    #[test]
    fn cache_entry_round_trips() {
        let temp_dir = tempfile::tempdir().expect("temporary cache directory");
        let path = temp_dir.path().join("nested").join("abi.json");
        let code_hash = "b".repeat(64);
        let entry = VerifierAbiCacheEntry {
            schema_version: VERIFIER_ABI_CACHE_SCHEMA_VERSION,
            code_hash: code_hash.clone(),
            fetched_at: 123,
            abi: ContractABI {
                contract_name: "CachedContract".to_owned(),
                ..ContractABI::default()
            },
        };

        write_cache_entry(&path, &entry).expect("write cache entry");

        let cached = read_cache_entry(&path, &code_hash).expect("read cache entry");
        assert_eq!(cached.code_hash, code_hash);
        assert_eq!(cached.fetched_at, 123);
        assert_eq!(cached.abi.contract_name, "CachedContract");
    }

    #[test]
    fn ignores_old_negative_cache_entries() {
        let temp_dir = tempfile::tempdir().expect("temporary cache directory");
        let path = temp_dir.path().join("abi.json");
        let code_hash = "c".repeat(64);
        fs::write(
            &path,
            serde_json::to_vec(&serde_json::json!({
                "schema_version": VERIFIER_ABI_CACHE_SCHEMA_VERSION,
                "code_hash": code_hash,
                "fetched_at": 123,
                "abi": null,
            }))
            .expect("serialize old cache entry"),
        )
        .expect("write old cache entry");

        assert!(read_cache_entry(&path, &code_hash).is_none());
    }
}
