// This cache tracks only compiler-visible source dependencies.
// On write, the compiler result supplies `source_map.files()`; we persist those
// paths and their content hash. On lookup, we rehash the same paths and reuse
// the entry only if nothing changed.
// Acton-level contract `depends` are handled by `build` before cache lookup:
// generated dependency files and rebuilt dependency contracts can force a
// downstream recompile without teaching this file about the contract graph.

use acton_config::config::{ActonConfig, project_root as configured_project_root};
use anyhow::{Result, anyhow};
use fs2::FileExt;
use log::debug;
use path_absolutize::Absolutize;
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::fs::File;
use std::fs::OpenOptions;
use std::io::{BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tolk_compiler::abi::ContractABI;
use tolk_compiler::compiler::CompilerResultSuccess;
use xxhash_rust::xxh3::Xxh3;

use crate::paths;

const CACHE_SCHEMA_VERSION: u32 = 13;
const CACHE_LOCK_WAIT_ATTEMPTS: usize = 60;
const CACHE_LOCK_RETRY_DELAY: Duration = Duration::from_secs(1);
const DEBUG_CACHE_SUBDIR: &str = "debug";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntry {
    pub code_boc64: String,
    pub code_hash_hex: String,
    pub fift_code: Option<String>,
    pub source_map: Option<tolk_compiler::SourceMap>,
    pub debug_marks_base64: Option<String>,
    pub symbol_types_json: Option<tolk_compiler::source_map::SymbolTypesJson>,
    pub debug_marks_json: Option<Vec<tolk_compiler::source_map::DebugMark>>,
    pub abi: Option<ContractABI>,
    pub dependency_paths: Vec<String>,
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

        Ok(Self {
            cache_dir,
            config,
            file_hash_cache: FxHashMap::default(),
            project_root,
            _temp_dir: None,
        })
    }

    pub fn temporary_for_project(project_root: PathBuf, config: ActonConfig) -> Result<Self> {
        let tmp_dir = tempfile::TempDir::new()?;

        Ok(Self {
            cache_dir: tmp_dir.path().to_path_buf(),
            config,
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

        if let Ok(dependencies) = self.dependency_paths_for_cache_lookup(file_path, &entry) {
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
        let dependencies = self.dependency_paths_from_compiler_result(file_path, result);
        debug!("Put new cache entry `{file_path}` with dependencies: {dependencies:?}");

        let dependencies_hash = self.compute_dependencies_hash(&dependencies)?;

        let entry = CacheEntry {
            code_boc64: result.code_boc64.clone(),
            code_hash_hex: result.code_hash_hex.clone(),
            fift_code: with_fift.then(|| result.fift_code.clone()),
            source_map: result.source_map.clone(),
            debug_marks_base64: result.debug_marks_base64.clone(),
            symbol_types_json: result.symbol_types_json.clone(),
            debug_marks_json: result.debug_marks_json.clone(),
            abi: result.abi.clone(),
            dependency_paths: dependencies,
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
        self.hash_mappings(&mut hasher);
        let result = hasher.finalize();
        hex::encode(result)
    }

    fn hash_mappings(&self, hasher: &mut Sha256) {
        if let Some(mappings) =
            acton_config::config::normalize_mappings(&self.config.mappings, &self.project_root)
        {
            for (prefix, target) in mappings {
                hasher.update(b"mapping");
                hasher.update(prefix.as_bytes());
                hasher.update(b"\0");
                hasher.update(target.as_bytes());
                hasher.update(b"\0");
            }
        }
    }

    fn dependency_paths_for_cache_lookup(
        &self,
        file_path: &str,
        entry: &CacheEntry,
    ) -> Result<Vec<String>> {
        if entry.dependency_paths.is_empty() {
            return Err(anyhow!("Cache entry does not include dependency paths"));
        }

        let normalized_entrypoint = self.normalize_dependency_path(file_path);
        if !entry
            .dependency_paths
            .iter()
            .any(|path| path == &normalized_entrypoint)
        {
            return Err(anyhow!(
                "Cache entry dependency paths do not include entrypoint {normalized_entrypoint}"
            ));
        }

        Ok(entry.dependency_paths.clone())
    }

    fn dependency_paths_from_compiler_result(
        &self,
        file_path: &str,
        result: &CompilerResultSuccess,
    ) -> Vec<String> {
        let dependencies = result
            .source_map
            .as_ref()
            .map(|source_map| {
                source_map
                    .files()
                    .iter()
                    .map(|file| self.normalize_dependency_path(&file.file_name))
                    .collect::<Vec<_>>()
            })
            .filter(|dependencies| !dependencies.is_empty())
            .unwrap_or_else(|| vec![self.normalize_dependency_path(file_path)]);

        Self::sort_and_dedup_paths(dependencies)
    }

    fn compute_dependencies_hash(&mut self, dependencies: &[String]) -> Result<String> {
        let mut hasher = Sha256::new();

        let mut normalized_deps: Vec<String> = dependencies
            .iter()
            .map(|dep| self.normalize_dependency_path(dep))
            .collect();
        normalized_deps = Self::sort_and_dedup_paths(normalized_deps);

        for dep_path in &normalized_deps {
            hasher.update(self.xxh3_128_dependency(dep_path)?);
            hasher.update(dep_path.as_bytes());
        }

        let result = hasher.finalize();
        Ok(hex::encode(result))
    }

    fn sort_and_dedup_paths(mut paths: Vec<String>) -> Vec<String> {
        paths.sort();
        paths.dedup();
        paths
    }

    fn is_virtual_dependency_path(path: &str) -> bool {
        path.starts_with("@stdlib/") || path.starts_with("@fiftlib/")
    }

    fn normalize_dependency_path(&self, path: &str) -> String {
        if Self::is_virtual_dependency_path(path) {
            path.to_string()
        } else {
            self.normalize_path(path)
        }
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

    fn xxh3_128_dependency(&mut self, path: &str) -> Result<[u8; 16]> {
        if let Some(content) = path
            .strip_prefix("@stdlib/")
            .and_then(tolk_compiler::compiler::read_stdlib_file)
        {
            return Ok(Self::xxh3_128_bytes(content.as_bytes()));
        }

        if let Some(content) = path
            .strip_prefix("@fiftlib/")
            .and_then(tolk_compiler::compiler::read_fift_stdlib_file)
        {
            return Ok(Self::xxh3_128_bytes(content.as_bytes()));
        }

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

    fn xxh3_128_bytes(bytes: &[u8]) -> [u8; 16] {
        let mut h = Xxh3::new();
        h.update(bytes);
        h.digest128().to_le_bytes()
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
            source_map: Some(source_map_for_paths(&[&main_path, &lib_path])),
            debug_marks_base64: None,
            symbol_types_json: None,
            debug_marks_json: None,
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
            source_map: Some(source_map_for_paths(&[&main_path])),
            debug_marks_base64: None,
            symbol_types_json: None,
            debug_marks_json: None,
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

    #[test]
    fn test_stdlib_dependency_hash_uses_embedded_content() {
        let temp_dir = tempdir().unwrap();
        let cache_dir = temp_dir.path().join("cache");
        let mut cache = FileBuildCache::new(Some(cache_dir)).expect("Failed to create cache");

        let hash = cache.compute_dependencies_hash(&["@stdlib/common.tolk".to_string()]);

        assert!(
            hash.is_ok(),
            "stdlib virtual dependency should be hashable without a filesystem path"
        );
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
            source_map: Some(source_map_for_paths(&[&main_path, &lib_path])),
            debug_marks_base64: None,
            symbol_types_json: None,
            debug_marks_json: None,
            abi: None,
        };

        cache.put(main_path.to_str().unwrap(), &result, false, false, 2, "1.1")?;

        let cached = cache.get(main_path.to_str().unwrap(), false, false, 2, "1.1");
        assert!(cached.is_some());
        let cached = cached.unwrap();
        assert_eq!(cached.code_boc64, "test_boc");
        assert_eq!(cached.fift_code, None);
        Ok((cache, lib_path, main_path))
    }

    fn source_map_for_paths(paths: &[&Path]) -> tolk_compiler::SourceMap {
        let files = paths
            .iter()
            .enumerate()
            .map(|(idx, path)| {
                serde_json::json!({
                    "file_id": idx + 1,
                    "file_name": path.to_string_lossy().to_string(),
                    "size_chars": fs::metadata(path).map_or(0, |metadata| metadata.len()),
                    "imports": [],
                })
            })
            .collect::<Vec<_>>();

        serde_json::from_value(serde_json::json!({
            "files": files,
            "unique_types": [],
            "struct_instantiations": [],
            "alias_instantiations": [],
            "declarations": [],
            "functions": [],
        }))
        .expect("test source map must deserialize")
    }
}
