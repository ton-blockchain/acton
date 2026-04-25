use acton_config::config::{ActonConfig, project_root as configured_project_root};
use anyhow::{Result, anyhow};
use fs2::FileExt;
use log::debug;
use path_absolutize::*;
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fs;
use std::fs::File;
use std::fs::OpenOptions;
use std::io::{BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tolk_compiler::abi::ContractABI;
use tolk_compiler::compiler::CompilerResultSuccess;
use ton_abi;
use xxhash_rust::xxh3::Xxh3;

use crate::paths;

const CACHE_SCHEMA_VERSION: u32 = 8;
const CACHE_LOCK_WAIT_ATTEMPTS: usize = 60;
const CACHE_LOCK_RETRY_DELAY: Duration = Duration::from_secs(1);
const DEBUG_CACHE_SUBDIR: &str = "debug";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntry {
    pub code_boc64: String,
    pub code_hash_hex: String,
    pub debug_mark_base64: Option<String>,
    pub fift_code: Option<String>,
    pub new_source_map: Option<tolk_compiler::SourceMap>,
    pub abi: Option<ContractABI>,
    pub dependencies_hash: String,
    pub timestamp: u64,
    pub schema_version: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FileSignature {
    modified_ns: u128,
    size: u64,
}

#[derive(Debug, Clone, Copy)]
struct FileHashCacheEntry {
    signature: FileSignature,
    hash: [u8; 16],
}

#[derive(Debug, Clone)]
struct DependenciesCacheEntry {
    signature: Option<FileSignature>,
    dependencies: Vec<String>,
}

struct CacheWriteLock {
    file: File,
}

impl Drop for CacheWriteLock {
    fn drop(&mut self) {
        let _ = FileExt::unlock(&self.file);
    }
}

#[derive(Debug)]
pub struct FileBuildCache {
    cache_dir: PathBuf,
    config: ActonConfig,
    contract_src_index: FxHashMap<String, String>,
    dependencies_cache: FxHashMap<String, DependenciesCacheEntry>,
    file_hash_cache: FxHashMap<String, FileHashCacheEntry>,
    project_root: PathBuf,
    _temp_dir: Option<tempfile::TempDir>,
}

impl FileBuildCache {
    pub fn new(cache_dir: Option<PathBuf>) -> Result<Self> {
        let project_root = configured_project_root().to_path_buf();
        let cache_dir = cache_dir.unwrap_or_else(|| paths::build_cache_dir(&project_root));

        if !cache_dir.exists() {
            fs::create_dir_all(&cache_dir)?;
        }

        let config = ActonConfig::load().unwrap_or_default();

        let contract_src_index = Self::build_contract_src_index(&config, &project_root);

        Ok(Self {
            cache_dir,
            config,
            contract_src_index,
            dependencies_cache: FxHashMap::default(),
            file_hash_cache: FxHashMap::default(),
            project_root,
            _temp_dir: None,
        })
    }

    pub fn temporary_for_project(project_root: PathBuf, config: ActonConfig) -> Result<Self> {
        let tmp_dir = tempfile::TempDir::new()?;
        let contract_src_index = Self::build_contract_src_index(&config, &project_root);

        Ok(Self {
            cache_dir: tmp_dir.path().to_path_buf(),
            config,
            contract_src_index,
            dependencies_cache: FxHashMap::default(),
            file_hash_cache: FxHashMap::default(),
            project_root,
            _temp_dir: Some(tmp_dir),
        })
    }

    fn read_cache_entry(path: &Path) -> Option<CacheEntry> {
        let file = File::open(path).ok()?;
        let cache_entry = serde_json::from_reader::<_, CacheEntry>(BufReader::new(file)).ok()?;
        (cache_entry.schema_version == CACHE_SCHEMA_VERSION).then_some(cache_entry)
    }

    fn debug_cache_dir(&self) -> PathBuf {
        self.cache_dir.join(DEBUG_CACHE_SUBDIR)
    }

    fn cache_file_path(&self, key: &str, with_debug_info: bool) -> PathBuf {
        if with_debug_info {
            self.debug_cache_dir().join(format!("{key}.json"))
        } else {
            self.cache_dir.join(format!("{key}.json"))
        }
    }

    fn legacy_cache_file_path(&self, key: &str) -> PathBuf {
        self.cache_dir.join(format!("{key}.json"))
    }

    fn acquire_write_lock(&self) -> Result<CacheWriteLock> {
        fs::create_dir_all(&self.cache_dir)?;

        let lock_file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .open(self.cache_dir.join(".lock"))?;

        for _ in 0..CACHE_LOCK_WAIT_ATTEMPTS {
            if lock_file.try_lock_exclusive().is_ok() {
                return Ok(CacheWriteLock { file: lock_file });
            }

            debug!("Cache directory currently write-locked, waiting...");
            thread::sleep(CACHE_LOCK_RETRY_DELAY);
        }

        Err(anyhow!(
            "Cache directory is locked by another process for more than 60 seconds"
        ))
    }

    fn clear_cache_dir_contents(&self) -> Result<()> {
        if !self.cache_dir.exists() {
            return Ok(());
        }

        for entry in fs::read_dir(&self.cache_dir)? {
            let entry = entry?;
            if entry.file_name() == ".lock" {
                continue;
            }

            let path = entry.path();
            let file_type = entry.file_type()?;
            if file_type.is_dir() {
                fs::remove_dir_all(path)?;
            } else {
                fs::remove_file(path)?;
            }
        }

        Ok(())
    }

    fn load_entry_for_key(&self, key: &str, with_debug_info: bool) -> Option<CacheEntry> {
        let primary_path = self.cache_file_path(key, with_debug_info);
        if let Some(entry) = Self::read_cache_entry(&primary_path) {
            return Some(entry);
        }

        if with_debug_info {
            return Self::read_cache_entry(&self.legacy_cache_file_path(key));
        }

        None
    }

    pub fn get(
        &mut self,
        file_path: &str,
        with_debug_info: bool,
        with_fift: bool,
        optimization_level: usize,
        tolk_version: &str,
    ) -> Option<CacheEntry> {
        let key = self.compute_key(
            file_path,
            with_debug_info,
            with_fift,
            optimization_level,
            tolk_version,
        );
        let entry = self.load_entry_for_key(&key, with_debug_info)?;
        let expected_dependencies_hash = entry.dependencies_hash.clone();

        if let Ok(dependencies) = self.get_dependencies(file_path) {
            debug!("Check hash `{file_path}` with dependencies: {dependencies:?}");
            if let Ok(current_hash) = self.compute_dependencies_hash(&dependencies)
                && current_hash == expected_dependencies_hash
            {
                return Some(entry);
            }
        }

        None
    }

    pub fn put(
        &mut self,
        file_path: &str,
        result: &CompilerResultSuccess,
        with_debug_info: bool,
        with_fift: bool,
        optimization_level: usize,
        tolk_version: &str,
    ) -> Result<()> {
        let dependencies = self.get_dependencies(file_path)?;
        debug!("Put new cache entry `{file_path}` with dependencies: {dependencies:?}");

        let dependencies_hash = self.compute_dependencies_hash(&dependencies)?;

        let entry = CacheEntry {
            code_boc64: result.code_boc64.clone(),
            code_hash_hex: result.code_hash_hex.clone(),
            debug_mark_base64: result.debug_mark_base64.clone(),
            fift_code: with_fift.then(|| result.fift_code.clone()),
            new_source_map: result.new_source_map.clone(),
            abi: result.abi.clone(),
            dependencies_hash,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            schema_version: CACHE_SCHEMA_VERSION,
        };

        let key = self.compute_key(
            file_path,
            with_debug_info,
            with_fift,
            optimization_level,
            tolk_version,
        );
        let cache_file = self.cache_file_path(&key, with_debug_info);
        let cache_parent = cache_file
            .parent()
            .map_or_else(|| self.cache_dir.clone(), Path::to_path_buf);

        let _lock = self.acquire_write_lock()?;
        fs::create_dir_all(&cache_parent)?;
        let content = serde_json::to_vec(&entry)?;
        let mut tmp = tempfile::NamedTempFile::new_in(&cache_parent)?;
        tmp.write_all(&content)?;
        tmp.flush()?;
        tmp.persist(&cache_file)
            .map_err(|err| anyhow!("Failed to persist cache entry {}: {}", key, err.error))?;

        Ok(())
    }

    fn compute_key(
        &self,
        file_path: &str,
        with_debug_info: bool,
        with_fift: bool,
        optimization_level: usize,
        tolk_version: &str,
    ) -> String {
        let mut hasher = Sha256::new();
        let normalized_path = self.normalize_path(file_path);
        hasher.update(normalized_path.as_bytes());
        hasher.update(CACHE_SCHEMA_VERSION.to_le_bytes());
        if with_debug_info {
            hasher.update(b"debug_info = true");
        }
        if with_fift {
            hasher.update(b"fift = true");
        }
        hasher.update(optimization_level.to_le_bytes());
        hasher.update(tolk_version.as_bytes());
        let result = hasher.finalize();
        hex::encode(result)
    }

    fn get_dependencies(&mut self, file_path: &str) -> Result<Vec<String>> {
        self.resolve_dependencies(file_path, &mut HashSet::new())
    }

    fn resolve_dependencies(
        &mut self,
        file_path: &str,
        visited: &mut HashSet<String>,
    ) -> Result<Vec<String>> {
        let normalized_path = self.normalize_path(file_path);
        let signature = Self::file_signature(normalized_path.as_str()).ok();
        let use_cache = visited.is_empty();
        if use_cache
            && let Some(cached) = self.dependencies_cache.get(&normalized_path)
            && cached.signature == signature
        {
            return Ok(cached.dependencies.clone());
        }

        let mappings = self.config.mappings();
        let file_deps = ton_abi::get_file_dependencies(file_path, true, &mappings)
            .map_err(|e| anyhow!("Failed to get file dependencies: {e}"))?;

        let Some(contract_name) = self.contract_src_index.get(&normalized_path).cloned() else {
            if use_cache {
                self.dependencies_cache.insert(
                    normalized_path,
                    DependenciesCacheEntry {
                        signature,
                        dependencies: file_deps.clone(),
                    },
                );
            }
            return Ok(file_deps);
        };

        let dep_sources = {
            let Some(contracts) = self.config.contracts.as_ref().map(|cfg| &cfg.contracts) else {
                if use_cache {
                    self.dependencies_cache.insert(
                        normalized_path,
                        DependenciesCacheEntry {
                            signature,
                            dependencies: file_deps.clone(),
                        },
                    );
                }
                return Ok(file_deps);
            };

            let Some(contract_info) = contracts.get(&contract_name) else {
                if use_cache {
                    self.dependencies_cache.insert(
                        normalized_path,
                        DependenciesCacheEntry {
                            signature,
                            dependencies: file_deps.clone(),
                        },
                    );
                }
                return Ok(file_deps);
            };

            let has_deps = contract_info
                .depends
                .as_ref()
                .is_some_and(|deps| !deps.is_empty());
            if !has_deps {
                debug!("Skipping deps processing for `{normalized_path}` in `get_dependencies`");
                debug!("Using file dependencies: {file_deps:?}");
                if use_cache {
                    self.dependencies_cache.insert(
                        normalized_path,
                        DependenciesCacheEntry {
                            signature,
                            dependencies: file_deps.clone(),
                        },
                    );
                }
                // fast path, no deps, no extra logic to find all dependencies of each dependency
                return Ok(file_deps);
            }

            let mut dep_sources = Vec::new();
            if let Some(deps) = &contract_info.depends {
                for dep in deps {
                    let dep_name = dep.name();

                    if !visited.insert(dep_name.to_owned()) {
                        // already visited
                        continue;
                    }

                    let contract_config = contracts
                        .get(dep_name)
                        .ok_or_else(|| anyhow!("Contract '{dep_name}' not found in Acton.toml"))?;

                    dep_sources.push(
                        contract_config
                            .absolute_source_path(&self.project_root)
                            .to_string_lossy()
                            .to_string(),
                    );
                }
            }
            dep_sources
        };

        let mut result = file_deps;
        for dep_source in dep_sources {
            result.append(&mut self.resolve_dependencies(dep_source.as_str(), visited)?);
        }

        if use_cache {
            self.dependencies_cache.insert(
                normalized_path,
                DependenciesCacheEntry {
                    signature,
                    dependencies: result.clone(),
                },
            );
        }

        Ok(result)
    }

    fn compute_dependencies_hash(&mut self, dependencies: &[String]) -> Result<String> {
        let mut hasher = Sha256::new();

        let mut normalized_deps: Vec<String> = dependencies
            .iter()
            .map(|dep| self.normalize_path(dep))
            .collect();
        normalized_deps.sort();
        normalized_deps.dedup();

        for dep_path in &normalized_deps {
            hasher.update(self.xxh3_128_file(dep_path)?);
            hasher.update(dep_path.as_bytes());
        }

        let result = hasher.finalize();
        Ok(hex::encode(result))
    }

    fn normalize_path(&self, path: &str) -> String {
        Path::new(path)
            .absolutize_from(&self.project_root)
            .unwrap_or_else(|_| Path::new(path).into())
            .to_string_lossy()
            .to_string()
    }

    pub fn clear(&mut self) -> Result<()> {
        let _lock = self.acquire_write_lock()?;
        fs::create_dir_all(&self.cache_dir)?;
        self.clear_cache_dir_contents()?;
        self.dependencies_cache.clear();
        self.file_hash_cache.clear();

        Ok(())
    }

    #[must_use]
    pub fn size(&self) -> usize {
        let root_count = fs::read_dir(&self.cache_dir)
            .ok()
            .into_iter()
            .flatten()
            .flatten()
            .filter(|entry| entry.path().extension().and_then(|s| s.to_str()) == Some("json"))
            .count();
        let debug_count = fs::read_dir(self.debug_cache_dir())
            .ok()
            .into_iter()
            .flatten()
            .flatten()
            .filter(|entry| entry.path().extension().and_then(|s| s.to_str()) == Some("json"))
            .count();

        root_count + debug_count
    }

    fn build_contract_src_index(
        config: &ActonConfig,
        project_root: &Path,
    ) -> FxHashMap<String, String> {
        let mut index = FxHashMap::default();
        let Some(contracts) = config.contracts.as_ref().map(|cfg| &cfg.contracts) else {
            return index;
        };

        for (name, contract) in contracts {
            let abs_path = contract
                .absolute_source_path(project_root)
                .to_string_lossy()
                .to_string();
            index.insert(abs_path, name.clone());
        }

        index
    }

    fn file_signature(path: &str) -> Result<FileSignature> {
        let metadata = fs::metadata(path)?;
        let modified_ns = metadata
            .modified()
            .ok()
            .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
            .map_or(0, |duration| duration.as_nanos());
        Ok(FileSignature {
            modified_ns,
            size: metadata.len(),
        })
    }

    fn xxh3_128_file(&mut self, path: &str) -> Result<[u8; 16]> {
        let signature = Self::file_signature(path)?;
        if let Some(cached) = self.file_hash_cache.get(path)
            && cached.signature == signature
        {
            return Ok(cached.hash);
        }

        let mut h = Xxh3::new();
        let mut f = BufReader::new(File::open(path)?);
        let mut buf = [0u8; 64 * 1024];
        loop {
            let n = f.read(&mut buf)?;
            if n == 0 {
                break;
            }
            h.update(&buf[..n]);
        }
        let hash = h.digest128().to_le_bytes();
        self.file_hash_cache
            .insert(path.to_string(), FileHashCacheEntry { signature, hash });
        Ok(hash)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::{TempDir, tempdir};

    #[test]
    fn test_file_cache_operations() {
        let temp_dir = tempdir().unwrap();
        let (mut cache, lib_path, main_path) =
            prepare_cache(&temp_dir).expect("Failed to prepare cache");

        thread::sleep(Duration::from_millis(10));
        File::create(&lib_path)
            .unwrap()
            .write_all(b"fun helper() { return 1; }")
            .unwrap();

        let cached = cache.get(main_path.to_str().unwrap(), false, false, 2, "1.1");
        assert!(
            cached.is_none(),
            "Cache should be invalidated when dependency changes"
        );
    }

    #[test]
    fn test_should_return_none_for_different_debug_info() {
        let temp_dir = tempdir().unwrap();
        let (mut cache, _, main_path) = prepare_cache(&temp_dir).expect("Failed to prepare cache");

        let cached = cache.get(main_path.to_str().unwrap(), true, false, 2, "1.1");
        assert!(
            cached.is_none(),
            "Cache should be none since debug info mismatch"
        );
    }

    #[test]
    fn test_should_return_none_for_different_optimization_level() {
        let temp_dir = tempdir().unwrap();
        let (mut cache, _, main_path) = prepare_cache(&temp_dir).expect("Failed to prepare cache");

        let cached = cache.get(main_path.to_str().unwrap(), false, false, 0, "1.1");
        assert!(
            cached.is_none(),
            "Cache should be none since optimization level mismatch"
        );
    }

    #[test]
    fn test_should_return_none_for_different_tolk_version() {
        let temp_dir = tempdir().unwrap();
        let (mut cache, _, main_path) = prepare_cache(&temp_dir).expect("Failed to prepare cache");

        let cached = cache.get(main_path.to_str().unwrap(), false, false, 2, "1.2");
        assert!(
            cached.is_none(),
            "Cache should be none since Tolk version mismatch"
        );
    }

    #[test]
    fn test_corrupted_cache_entry_returns_none() {
        let temp_dir = tempdir().unwrap();
        let (mut cache, _, main_path) = prepare_cache(&temp_dir).expect("Failed to prepare cache");
        let key = cache.compute_key(main_path.to_str().unwrap(), false, false, 2, "1.1");
        let cache_file = cache.cache_file_path(&key, false);

        fs::write(cache_file, "corrupted cache data").unwrap();

        let cached = cache.get(main_path.to_str().unwrap(), false, false, 2, "1.1");
        assert!(cached.is_none(), "Corrupted cache entry should be ignored");
    }

    #[test]
    fn test_new_does_not_eagerly_clean_unrelated_corrupted_entries() {
        let temp_dir = tempdir().unwrap();
        let cache_dir = temp_dir.path().join("cache");
        fs::create_dir_all(&cache_dir).unwrap();

        let corrupted = cache_dir.join("broken.json");
        fs::write(&corrupted, "not-json").unwrap();

        let _cache = FileBuildCache::new(Some(cache_dir)).expect("Failed to create cache");

        assert!(
            corrupted.exists(),
            "Cache initialization should not scan and mutate unrelated entries"
        );
    }

    #[test]
    fn test_debug_entries_are_retained() {
        let temp_dir = tempdir().unwrap();
        let debug_dir = temp_dir.path().join(DEBUG_CACHE_SUBDIR);
        fs::create_dir_all(&debug_dir).unwrap();

        let first = debug_dir.join("first.json");
        fs::write(&first, "1111").unwrap();
        let second = debug_dir.join("second.json");
        fs::write(&second, "2222").unwrap();

        assert!(first.exists(), "First debug cache entry should remain");
        assert!(second.exists(), "Second debug cache entry should remain");
    }

    #[test]
    fn test_debug_entries_use_debug_subdirectory() {
        let temp_dir = tempdir().unwrap();
        let cache_dir = temp_dir.path().join("cache");
        let mut cache = FileBuildCache::new(Some(cache_dir)).expect("Failed to create cache");

        let lib_path = temp_dir.path().join("lib.tolk");
        File::create(&lib_path)
            .unwrap()
            .write_all(b"fun helper() { }")
            .unwrap();

        let main_path = temp_dir.path().join("main.tolk");
        File::create(&main_path)
            .unwrap()
            .write_all(b"import \"lib\";\nfun main() { }")
            .unwrap();

        let result = CompilerResultSuccess {
            fift_code: "test_fift_code".to_string(),
            code_boc64: "test_boc".to_string(),
            code_hash_hex: "test_hash".to_string(),
            debug_mark_base64: Some("test_debug_marks".to_string()),
            new_source_map: None,
            abi: None,
        };

        cache
            .put(main_path.to_str().unwrap(), &result, true, true, 2, "1.1")
            .expect("Failed to write debug cache entry");

        let key = cache.compute_key(main_path.to_str().unwrap(), true, true, 2, "1.1");
        assert!(
            cache.cache_file_path(&key, true).exists(),
            "Debug cache entry should be stored in debug subdirectory"
        );
        assert!(
            !cache.legacy_cache_file_path(&key).exists(),
            "Debug cache entry should not be written into the root cache namespace"
        );
        assert!(
            cache
                .get(main_path.to_str().unwrap(), true, true, 2, "1.1")
                .is_some(),
            "Debug cache entry should be readable back"
        );
    }

    #[test]
    fn test_fift_cache_key_is_separate_and_only_stored_when_requested() {
        let temp_dir = tempdir().unwrap();
        let cache_dir = temp_dir.path().join("cache");
        let mut cache = FileBuildCache::new(Some(cache_dir)).expect("Failed to create cache");

        let main_path = temp_dir.path().join("main.tolk");
        File::create(&main_path)
            .unwrap()
            .write_all(b"fun main() { }")
            .unwrap();

        let result = CompilerResultSuccess {
            fift_code: "test_fift_code".to_string(),
            code_boc64: "test_boc".to_string(),
            code_hash_hex: "test_hash".to_string(),
            debug_mark_base64: None,
            new_source_map: None,
            abi: None,
        };

        cache
            .put(main_path.to_str().unwrap(), &result, false, false, 2, "1.1")
            .expect("Failed to write non-fift cache entry");

        let no_fift = cache
            .get(main_path.to_str().unwrap(), false, false, 2, "1.1")
            .expect("non-fift cache entry should exist");
        assert_eq!(no_fift.fift_code, None);

        let with_fift_before = cache.get(main_path.to_str().unwrap(), false, true, 2, "1.1");
        assert!(
            with_fift_before.is_none(),
            "fift-enabled lookup should miss when only non-fift cache entry exists"
        );

        cache
            .put(main_path.to_str().unwrap(), &result, false, true, 2, "1.1")
            .expect("Failed to write fift cache entry");

        let with_fift = cache
            .get(main_path.to_str().unwrap(), false, true, 2, "1.1")
            .expect("fift cache entry should exist");
        assert_eq!(with_fift.fift_code.as_deref(), Some("test_fift_code"));
    }

    fn prepare_cache(temp_dir: &TempDir) -> Result<(FileBuildCache, PathBuf, PathBuf)> {
        let cache_dir = temp_dir.path().join("cache");

        let mut cache = FileBuildCache::new(Some(cache_dir))?;

        let lib_path = temp_dir.path().join("lib.tolk");
        File::create(&lib_path)?.write_all(b"fun helper() { }")?;

        let main_path = temp_dir.path().join("main.tolk");
        File::create(&main_path)?.write_all(b"import \"lib\";\nfun main() { }")?;

        let result = CompilerResultSuccess {
            fift_code: "test_fift_code".to_string(),
            code_boc64: "test_boc".to_string(),
            code_hash_hex: "test_hash".to_string(),
            debug_mark_base64: Some("test_debug_marks".to_string()),
            new_source_map: None,
            abi: None,
        };

        cache.put(main_path.to_str().unwrap(), &result, false, false, 2, "1.1")?;

        let cached = cache.get(main_path.to_str().unwrap(), false, false, 2, "1.1");
        assert!(cached.is_some());
        let cached = cached.unwrap();
        assert_eq!(cached.code_boc64, "test_boc");
        assert_eq!(
            cached.debug_mark_base64.as_deref(),
            Some("test_debug_marks")
        );
        assert_eq!(cached.fift_code, None);
        Ok((cache, lib_path, main_path))
    }
}
