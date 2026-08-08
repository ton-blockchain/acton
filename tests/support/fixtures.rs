use crate::common::acton_exe;
use crate::support::project::{ActonCommand, ProcessCommandBuilder};
use fs_extra::dir::{CopyOptions, copy};
use include_dir::{Dir, include_dir};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tempfile::TempDir;

pub(crate) struct FixtureProject {
    _tmp_dir: TempDir,
    project_path: PathBuf,
    isolated_home: PathBuf,
    enabled_slots: HashMap<String, Vec<usize>>,
}

#[allow(dead_code)]
impl FixtureProject {
    /// Load a fixture project from tests/projects/{name}
    pub(crate) fn load(name: &str) -> Self {
        let tmp_dir = Self::copy_fixture_project(name);
        let project_path = tmp_dir.path().join(name);
        Self::patch_imports(&project_path);

        let isolated_home = tmp_dir.path().join(".acton-test-home");
        fs::create_dir_all(&isolated_home).expect("Failed to create isolated home dir");

        Self {
            _tmp_dir: tmp_dir,
            project_path,
            isolated_home,
            enabled_slots: HashMap::new(),
        }
    }

    /// Enable a single slot in a file
    ///
    /// # Example
    /// ```
    /// FixtureProject::load("basic")
    ///     .with_slot("contracts/counter.tolk", 1)
    /// ```
    pub(crate) fn with_slot(mut self, file: &str, slot: usize) -> Self {
        let slots = self.enabled_slots.entry(file.to_string()).or_default();
        slots.push(slot);
        Self::enable_slot(&self.project_path, file, slot);
        self
    }

    /// Enable multiple slots in a file
    ///
    /// # Example
    /// ```
    /// FixtureProject::load("basic")
    ///     .with_slots("tests/counter.test.tolk", &[1, 2, 3])
    /// ```
    #[allow(dead_code)]
    pub(crate) fn with_slots(mut self, file: &str, slots: &[usize]) -> Self {
        for &slot in slots {
            let slot_list = self.enabled_slots.entry(file.to_string()).or_default();
            slot_list.push(slot);
            Self::enable_slot(&self.project_path, file, slot);
        }
        self
    }

    /// Enable a slot in contract file (shorthand)
    pub(crate) fn with_contract_slot(self, slot: usize) -> Self {
        self.with_slot("contracts/counter.tolk", slot)
    }

    /// Enable multiple contract slots (shorthand)
    #[allow(dead_code)]
    pub(crate) fn with_contract_slots(self, slots: &[usize]) -> Self {
        self.with_slots("contracts/counter.tolk", slots)
    }

    /// Enable a slot in test file (shorthand)
    pub(crate) fn with_test_slot(self, slot: usize) -> Self {
        self.with_slot("tests/counter.test.tolk", slot)
    }

    /// Enable multiple test slots (shorthand)
    #[allow(dead_code)]
    pub(crate) fn with_test_slots(self, slots: &[usize]) -> Self {
        self.with_slots("tests/counter.test.tolk", slots)
    }

    /// Replace template variables in all files
    ///
    /// # Example
    /// ```
    /// let mut vars = HashMap::new();
    /// vars.insert("VALUE", "100");
    /// FixtureProject::load("basic")
    ///     .with_template_vars(vars)
    /// ```
    #[allow(dead_code)]
    pub(crate) fn with_template_vars(self, vars: HashMap<&str, &str>) -> Self {
        for (key, value) in vars {
            self.replace_in_all_files(&format!("{{{{ {key} }}}}"), value);
        }
        self
    }

    /// Get `ActonCommand` builder for this project
    pub(crate) fn acton(&self) -> ActonCommand {
        let cmd = ProcessCommandBuilder::new(acton_exe())
            .env("HOME", &self.isolated_home)
            .env("USERPROFILE", &self.isolated_home)
            .env("ACTON_LOG_DIR", self.project_path.join(".acton-test-logs"));
        ActonCommand {
            cmd,
            project: Arc::new(crate::support::project::ProjectRef {
                path: self.project_path.clone(),
            }),
            test_paths: Vec::new(),
            filter: None,
            build_clear_cache: false,
            build_contract: None,
            build_graph: None,
            build_out_dir: None,
            build_gen_dir: None,
            build_output_abi: None,
            build_output_fift: None,
            disasm_string: None,
            disasm_output: None,
            disasm_address: None,
            disasm_api_key: None,
            disasm_net: None,
            disasm_follow_libraries: false,
            disasm_show_hashes: false,
            disasm_show_offsets: false,
            compile_json: false,
            compile_base64_only: false,
            compile_boc: None,
            compile_fift: None,
            compile_source_map: None,
            compile_allow_no_entrypoint: false,
            test_reporters: vec![],
            junit_merge: false,
            test_exclude_patterns: vec![],
            test_include_patterns: vec![],
            verify_contract: None,
            verify_address: None,
            verify_wallet: None,
            verify_network: None,
            verify_new: false,
            test_fail_fast: false,
            test_no_capture: false,
            script_fork_net: None,
            build_info: false,
            force_no_color_env: true,
            color_mode: None,
            wallet_secure_default_false: false,
        }
    }

    /// Get the project path
    #[allow(dead_code)]
    pub(crate) fn path(&self) -> &Path {
        &self.project_path
    }

    fn copy_fixture_project(name: &str) -> TempDir {
        static LIB_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/lib");

        let tmp = TempDir::new().expect("Failed to create temp dir");
        fs::create_dir_all(tmp.path().join("lib")).expect("Failed to create lib dir");
        LIB_DIR
            .extract(tmp.path().join("lib"))
            .expect("Failed to extract lib");

        let fixture_dir = Path::new("tests/projects").join(name);

        let mut opts = CopyOptions::new();
        opts.copy_inside = true;

        copy(&fixture_dir, tmp.path(), &opts).expect("Failed to copy fixture project");
        let copied_project_path = tmp.path().join(name);
        let copied_acton_dir = copied_project_path.join(".acton");
        if copied_acton_dir.exists() {
            fs::remove_dir_all(&copied_acton_dir)
                .expect("Failed to remove copied fixture .acton directory");
        }

        tmp
    }

    fn patch_imports(project_path: &Path) {
        let test_file = project_path.join("tests/counter.test.tolk");
        if test_file.exists() {
            let content = fs::read_to_string(&test_file).expect("Failed to read test file");
            let new_content = content.replace("../../../../", "../../");
            fs::write(&test_file, new_content).expect("Failed to write test file");
        }
    }

    fn enable_slot(project_path: &Path, file: &str, index: usize) {
        let file_path = project_path.join(file);
        assert!(
            file_path.exists(),
            "File {file} does not exist in fixture project"
        );

        let content =
            fs::read_to_string(&file_path).unwrap_or_else(|_| panic!("Failed to read file {file}"));
        let new_content = content.replace(&format!("// SLOT_{index}: "), "");
        fs::write(&file_path, new_content)
            .unwrap_or_else(|_| panic!("Failed to write file {file}"));
    }

    #[allow(dead_code)]
    fn replace_in_all_files(&self, from: &str, to: &str) {
        use walkdir::WalkDir;

        for entry in WalkDir::new(&self.project_path)
            .into_iter()
            .filter_map(Result::ok)
        {
            if entry.file_type().is_file()
                && let Some(ext) = entry.path().extension()
                && (ext == "tolk" || ext == "toml")
            {
                let content = fs::read_to_string(entry.path()).expect("Failed to read file");
                if content.contains(from) {
                    let new_content = content.replace(from, to);
                    fs::write(entry.path(), new_content).expect("Failed to write file");
                }
            }
        }
    }
}
