use acton_config::config::ActonConfig;
use anyhow::{Result, anyhow};
use fs2::FileExt;
use log::debug;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;
use tolkc::abi::ContractABI;
use tolkc::compiler::CompilerResultSuccess;
use ton_abi;
use ton_source_map::SourceMap;

const CACHE_SCHEMA_VERSION: u32 = 2;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntry {
    pub code_boc64: String,
    pub code_hash_hex: String,
    pub fift_code: String,
    pub source_map: Option<SourceMap>,
    pub abi: Option<ContractABI>,
    pub dependencies_hash: String,
    pub timestamp: u64,
    pub schema_version: u32,
}

#[derive(Debug)]
pub struct FileBuildCache {
    cache_dir: PathBuf,
    config: ActonConfig,
    entries: HashMap<String, CacheEntry>,
    _lock_file: File,
}

impl FileBuildCache {
    pub fn new(cache_dir: Option<PathBuf>) -> Result<Self> {
        let cache_dir = cache_dir.unwrap_or_else(|| PathBuf::from(".acton/cache"));

        if !cache_dir.exists() {
            fs::create_dir_all(&cache_dir)?;
        }

        let lock_file_path = cache_dir.join(".lock");
        let lock_file = File::create(&lock_file_path)?;

        let mut locked = false;
        for _ in 0..60 {
            if lock_file.try_lock_exclusive().is_ok() {
                locked = true;
                break;
            }

            debug!("{}", "Cache directory currently locked, waiting...");
            thread::sleep(Duration::from_millis(1000));
        }

        if !locked {
            return Err(anyhow!(
                "Cache directory is locked by another process for more than 60 seconds"
            ));
        }

        let entries = Self::load_cache(&cache_dir)?;

        let config = ActonConfig::load().unwrap_or_default();

        Ok(Self {
            cache_dir,
            entries,
            config,
            _lock_file: lock_file,
        })
    }

    pub fn dummy() -> Result<Self> {
        let tmp_dir = tempfile::TempDir::new()?;
        let config = ActonConfig::load().unwrap_or_default();

        let lock_file_path = tmp_dir.path().join(".lock");
        let lock_file = File::create(&lock_file_path)?;

        Ok(Self {
            cache_dir: tmp_dir.path().to_path_buf(),
            entries: HashMap::new(),
            config,
            _lock_file: lock_file,
        })
    }

    fn load_cache(cache_dir: &Path) -> Result<HashMap<String, CacheEntry>> {
        let mut entries = HashMap::new();

        if !cache_dir.exists() {
            return Ok(entries);
        }

        for entry in fs::read_dir(cache_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }

            let Some(file_stem) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };

            match fs::read_to_string(&path) {
                Ok(content) => match serde_json::from_str::<CacheEntry>(&content) {
                    Ok(cache_entry) => {
                        if cache_entry.schema_version != CACHE_SCHEMA_VERSION {
                            let _ = fs::remove_file(&path);
                            continue;
                        }
                        entries.insert(file_stem.to_string(), cache_entry);
                    }
                    Err(_) => {
                        let _ = fs::remove_file(&path);
                    }
                },
                Err(_) => {
                    let _ = fs::remove_file(&path);
                }
            }
        }

        Ok(entries)
    }

    pub fn get(
        &mut self,
        file_path: &str,
        with_debug_info: bool,
        optimization_level: usize,
        tolk_version: String,
    ) -> Option<CacheEntry> {
        let key = self.compute_key(file_path, with_debug_info, optimization_level, tolk_version);
        let entry = self.entries.get(&key)?;

        if let Ok(dependencies) = self.get_dependencies(file_path, &mut HashSet::new()) {
            debug!("Check hash `{file_path}` with dependencies: {dependencies:?}");
            if let Ok(current_hash) = self.compute_dependencies_hash(&dependencies)
                && current_hash == entry.dependencies_hash
            {
                return Some(entry.clone());
            }
        }

        None
    }

    pub fn put(
        &mut self,
        file_path: &str,
        result: &CompilerResultSuccess,
        with_debug_info: bool,
        optimization_level: usize,
        tolk_version: String,
    ) -> Result<()> {
        let dependencies = self.get_dependencies(file_path, &mut HashSet::new())?;
        debug!("Put new cache entry `{file_path}` with dependencies: {dependencies:?}");

        let dependencies_hash = self.compute_dependencies_hash(&dependencies)?;

        let entry = CacheEntry {
            code_boc64: result.code_boc64.clone(),
            code_hash_hex: result.code_hash_hex.clone(),
            fift_code: result.fift_code.clone(),
            source_map: result.source_map.clone(),
            abi: result.abi.clone(),
            dependencies_hash,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            schema_version: CACHE_SCHEMA_VERSION,
        };

        let key = self.compute_key(file_path, with_debug_info, optimization_level, tolk_version);
        let cache_file = self.cache_dir.join(format!("{key}.json"));

        let content = serde_json::to_string_pretty(&entry)?;
        let tmp = cache_file.with_extension("json.tmp");
        fs::write(&tmp, content)?;
        fs::rename(&tmp, cache_file)?;

        self.entries.insert(key, entry);

        Ok(())
    }

    fn compute_key(
        &self,
        file_path: &str,
        with_debug_info: bool,
        optimization_level: usize,
        tolk_version: String,
    ) -> String {
        let mut hasher = Sha256::new();
        let normalized_path = self.normalize_path(file_path);
        hasher.update(normalized_path.as_bytes());
        hasher.update(CACHE_SCHEMA_VERSION.to_le_bytes());
        if with_debug_info {
            hasher.update(b"debug_info = true");
        }
        hasher.update(optimization_level.to_le_bytes());
        hasher.update(tolk_version.into_bytes());
        let result = hasher.finalize();
        hex::encode(result)
    }

    fn get_dependencies(
        &self,
        file_path: &str,
        visited: &mut HashSet<String>,
    ) -> Result<Vec<String>> {
        let file_deps = ton_abi::get_file_dependencies(file_path, true, &self.config.mappings)
            .map_err(|e| anyhow!("Failed to get file dependencies: {e}"))?;

        let file_path =
            dunce::canonicalize(file_path).unwrap_or_else(|_| PathBuf::from(&file_path));
        let contracts = self.config.contracts.clone().unwrap_or_default().contracts;
        let Some((_, contract_info)) = contracts.iter().find(|(_, config)| {
            dunce::canonicalize(&config.src).unwrap_or_else(|_| PathBuf::from(&config.src))
                == file_path
        }) else {
            return Ok(file_deps);
        };

        let has_deps = contract_info
            .depends
            .as_ref()
            .is_some_and(|deps| !deps.is_empty());
        if !has_deps {
            debug!(
                "Skipping deps processing for `{}` in `get_dependencies`",
                file_path.display()
            );
            debug!("Using file dependencies: {file_deps:?}",);
            // fast path, no deps, no extra logic to find all dependencies of each dependency
            return Ok(file_deps);
        }

        let mut result = file_deps;

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

                result.append(&mut self.get_dependencies(contract_config.src.as_str(), visited)?);
            }
        }

        Ok(result)
    }

    fn compute_dependencies_hash(&self, dependencies: &[String]) -> Result<String> {
        let mut hasher = Sha256::new();

        let mut normalized_deps: Vec<String> = dependencies
            .iter()
            .map(|dep| self.normalize_path(dep))
            .collect();
        normalized_deps.sort();
        normalized_deps.dedup();

        for dep_path in &normalized_deps {
            if fs::metadata(dep_path).is_ok() {
                hasher.update(Self::sha256_file(dep_path)?);
            } else {
                hasher.update(b"FILE_NOT_FOUND:");
            }
            hasher.update(dep_path.as_bytes());
        }

        let result = hasher.finalize();
        Ok(hex::encode(result))
    }

    fn normalize_path(&self, path: &str) -> String {
        let path_buf = PathBuf::from(path);

        if path_buf.exists() {
            path_buf
                .canonicalize()
                .unwrap_or(path_buf)
                .to_string_lossy()
                .to_string()
        } else {
            path.to_string()
        }
    }

    pub fn clear(&mut self) -> Result<()> {
        let _ = FileExt::unlock(&self._lock_file);

        if self.cache_dir.exists() {
            fs::remove_dir_all(&self.cache_dir)?;
            fs::create_dir_all(&self.cache_dir)?;
        }
        self.entries.clear();

        let lock_file_path = self.cache_dir.join(".lock");
        self._lock_file = File::create(&lock_file_path)?;

        let mut locked = false;
        for _ in 0..50 {
            if self._lock_file.try_lock_exclusive().is_ok() {
                locked = true;
                break;
            }
            thread::sleep(Duration::from_millis(100));
        }

        if !locked {
            return Err(anyhow!(
                "Failed to re-lock cache directory after clearing (waited 5 seconds)"
            ));
        }

        Ok(())
    }

    #[must_use]
    pub fn size(&self) -> usize {
        self.entries.len()
    }

    fn sha256_file(path: &str) -> Result<[u8; 32]> {
        let mut h = Sha256::new();
        let mut f = BufReader::new(File::open(path)?);
        let mut buf = [0u8; 64 * 1024];
        loop {
            let n = f.read(&mut buf)?;
            if n == 0 {
                break;
            }
            h.update(&buf[..n]);
        }
        Ok(h.finalize().into())
    }
}

impl Drop for FileBuildCache {
    fn drop(&mut self) {
        let _ = FileExt::unlock(&self._lock_file);
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

        let cached = cache.get(main_path.to_str().unwrap(), false, 2, "1.1".to_string());
        assert!(
            cached.is_none(),
            "Cache should be invalidated when dependency changes"
        );
    }

    #[test]
    fn test_should_return_none_for_different_debug_info() {
        let temp_dir = tempdir().unwrap();
        let (mut cache, _, main_path) = prepare_cache(&temp_dir).expect("Failed to prepare cache");

        let cached = cache.get(main_path.to_str().unwrap(), true, 2, "1.1".to_string());
        assert!(
            cached.is_none(),
            "Cache should be none since debug info mismatch"
        );
    }

    #[test]
    fn test_should_return_none_for_different_optimization_level() {
        let temp_dir = tempdir().unwrap();
        let (mut cache, _, main_path) = prepare_cache(&temp_dir).expect("Failed to prepare cache");

        let cached = cache.get(main_path.to_str().unwrap(), false, 0, "1.1".to_string());
        assert!(
            cached.is_none(),
            "Cache should be none since optimization level mismatch"
        );
    }

    #[test]
    fn test_should_return_none_for_different_tolk_version() {
        let temp_dir = tempdir().unwrap();
        let (mut cache, _, main_path) = prepare_cache(&temp_dir).expect("Failed to prepare cache");

        let cached = cache.get(main_path.to_str().unwrap(), false, 2, "1.2".to_string());
        assert!(
            cached.is_none(),
            "Cache should be none since Tolk version mismatch"
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
            source_map: None,
            abi: None,
        };

        cache.put(
            main_path.to_str().unwrap(),
            &result,
            false,
            2,
            "1.1".to_string(),
        )?;

        let cached = cache.get(main_path.to_str().unwrap(), false, 2, "1.1".to_string());
        assert!(cached.is_some());
        assert_eq!(cached.unwrap().code_boc64, "test_boc");
        Ok((cache, lib_path, main_path))
    }
}
