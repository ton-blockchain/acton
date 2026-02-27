use crate::common::{acton_exe, assert_ui};
use crate::support::assertions::TestOutput;
use crate::support::tmp::create_tmp_dir;
use acton_config::color::ColorMode;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tempfile::TempDir;

pub(crate) struct ProjectBuilder {
    name: String,
    temp_dir: TempDir,
    contracts: Vec<ContractDef>,
    tests: Vec<(String, String)>,
    files: Vec<(String, String)>,
    raw_files: Vec<(String, String)>,
    scripts: Vec<(String, String)>,
    mappings: Vec<(String, String)>,
    lint_levels: BTreeMap<String, String>,
    lint_excludes: Vec<String>,
    lint_max_warnings: Option<usize>,
    lint_sarif_path: Option<String>,
    test_config: Option<TestConfig>,
    license: Option<String>,
    create_acton_toml: bool,
}

struct ContractDef {
    name: String,
    code: ContractSource,
    depends: Vec<DependencyDef>,
    output: Option<String>,
    dir: Option<String>,
}

enum ContractSource {
    Tolk(String), // Tolk source code
    Boc(Vec<u8>), // Raw BoC bytes
}

#[derive(Clone)]
struct DependencyDef {
    name: String,
    kind: Option<String>,     // "embed_code" or "library_ref"
    function: Option<String>, // custom function name
    path: Option<String>,     // custom output path
}

#[derive(Clone, Default)]
pub(crate) struct TestConfig {
    pub filter: Option<String>,
    pub exclude_patterns: Option<Vec<String>>,
    pub include_patterns: Option<Vec<String>>,
    pub reporters: Option<Vec<String>>,
    pub debug: Option<bool>,
    pub debug_port: Option<u16>,
    pub backtrace: Option<String>,
    pub coverage: Option<bool>,
    pub coverage_format: Option<String>,
    pub coverage_file: Option<String>,
    pub junit_path: Option<String>,
    pub junit_merge: Option<bool>,
    pub fail_fast: Option<bool>,
}

#[allow(dead_code)]
impl ProjectBuilder {
    pub(crate) fn new(name: &str) -> Self {
        let temp_dir = create_tmp_dir();
        Self {
            name: name.to_string(),
            temp_dir,
            contracts: Vec::new(),
            tests: Vec::new(),
            files: Vec::new(),
            raw_files: Vec::new(),
            scripts: Vec::new(),
            mappings: Vec::new(),
            lint_levels: BTreeMap::new(),
            lint_excludes: Vec::new(),
            lint_max_warnings: None,
            lint_sarif_path: None,
            test_config: None,
            license: Some("MIT".to_string()),
            create_acton_toml: true,
        }
    }

    /// Don't create Acton.toml (useful for testing init command)
    pub(crate) fn without_acton_toml(mut self) -> Self {
        self.create_acton_toml = false;
        self
    }

    /// Set project license (for gen file headers)
    ///
    /// # Examples
    /// ```
    /// .with_license(Some("Apache-2.0"))  // Set custom license
    /// .with_license(None)                 // No license header
    /// ```
    pub(crate) fn with_license(mut self, license: Option<&str>) -> Self {
        self.license = license.map(ToString::to_string);
        self
    }

    pub(crate) fn script_config(mut self, name: &str, command: &str) -> Self {
        self.scripts.push((name.to_string(), command.to_string()));
        self
    }

    pub(crate) fn mapping(mut self, prefix: &str, target: &str) -> Self {
        self.mappings.push((prefix.to_string(), target.to_string()));
        self
    }

    /// Configure lint rule level in Acton.toml.
    ///
    /// # Examples
    /// ```
    /// .with_lint_level("unauthorized-access", "warn")
    /// .with_lint_level("unused-variable", "deny")
    /// ```
    pub(crate) fn with_lint_level(mut self, rule: &str, level: &str) -> Self {
        self.lint_levels.insert(rule.to_string(), level.to_string());
        self
    }

    /// Exclude files from lint checks using glob patterns.
    ///
    /// # Examples
    /// ```
    /// .with_lint_exclude("contracts/generated/*.tolk")
    /// .with_lint_exclude("contracts/legacy.tolk")
    /// ```
    pub(crate) fn with_lint_exclude(mut self, pattern: &str) -> Self {
        self.lint_excludes.push(pattern.to_string());
        self
    }

    /// Configure maximum allowed warning count for `acton check`.
    pub(crate) fn with_lint_max_warnings(mut self, max_warnings: usize) -> Self {
        self.lint_max_warnings = Some(max_warnings);
        self
    }

    /// Configure path for SARIF report output from `acton check`.
    pub(crate) fn with_lint_sarif_path(mut self, path: &str) -> Self {
        self.lint_sarif_path = Some(path.to_string());
        self
    }

    pub(crate) fn contract(mut self, name: &str, code: &str) -> Self {
        self.contracts.push(ContractDef {
            name: name.to_string(),
            code: ContractSource::Tolk(code.to_string()),
            depends: Vec::new(),
            output: None,
            dir: None,
        });
        self
    }

    /// Add a contract file in a custom directory
    ///
    /// Useful for testing `init` command discovery with contracts in non-standard locations.
    /// The file will be created at `{directory}/{name}.tolk`.
    ///
    /// # Examples
    /// ```
    /// .contract_at("wallet", "src/contracts", CONTRACT_CODE)  // Creates src/contracts/wallet.tolk
    /// .contract_at("nested", "contracts/nested", CONTRACT_CODE)  // Creates contracts/nested/nested.tolk
    /// ```
    pub(crate) fn contract_at(mut self, name: &str, directory: &str, code: &str) -> Self {
        self.contracts.push(ContractDef {
            name: name.to_string(),
            code: ContractSource::Tolk(code.to_string()),
            depends: Vec::new(),
            output: None,
            dir: Some(directory.to_string()),
        });
        self
    }

    /// Add a contract from a `BoC` file
    ///
    /// # Examples
    /// ```
    /// .contract_from_boc("precompiled", boc_bytes)
    /// ```
    pub(crate) fn contract_from_boc(mut self, name: &str, boc_data: Vec<u8>) -> Self {
        self.contracts.push(ContractDef {
            name: name.to_string(),
            code: ContractSource::Boc(boc_data),
            depends: Vec::new(),
            output: None,
            dir: None,
        });
        self
    }

    /// Add a contract with simple dependencies (default `EmbedCode`)
    ///
    /// # Examples
    /// ```
    /// .contract_with_deps("simple", CONTRACT_CODE, vec!["child"])
    /// ```
    #[allow(clippy::needless_pass_by_value)]
    pub(crate) fn contract_with_deps(mut self, name: &str, code: &str, depends: Vec<&str>) -> Self {
        self.contracts.push(ContractDef {
            name: name.to_string(),
            code: ContractSource::Tolk(code.to_string()),
            depends: depends
                .iter()
                .map(|s| DependencyDef {
                    name: (*s).to_string(),
                    kind: None,
                    function: None,
                    path: None,
                })
                .collect(),
            output: None,
            dir: None,
        });
        self
    }

    /// Add a contract with detailed dependency configuration
    ///
    /// # Examples
    /// ```
    /// .contract_with_detailed_deps("main", CODE, vec![
    ///     ("child", Some("library_ref"), None, None),
    ///     ("utils", Some("embed_code"), Some("customFunc"), None),
    /// ])
    /// ```
    #[allow(clippy::type_complexity)]
    pub(crate) fn contract_with_detailed_deps(
        mut self,
        name: &str,
        code: &str,
        depends: Vec<(&str, Option<&str>, Option<&str>, Option<&str>)>,
    ) -> Self {
        self.contracts.push(ContractDef {
            name: name.to_string(),
            code: ContractSource::Tolk(code.to_string()),
            depends: depends
                .iter()
                .map(|(dep_name, kind, function, path)| DependencyDef {
                    name: (*dep_name).to_string(),
                    kind: kind.map(ToString::to_string),
                    function: function.map(ToString::to_string),
                    path: path.map(ToString::to_string),
                })
                .collect(),
            output: None,
            dir: None,
        });
        self
    }

    /// Add a contract with `BoC` output
    ///
    /// # Examples
    /// ```
    /// .contract_with_output("simple", CONTRACT_CODE, "simple.boc")
    /// ```
    pub(crate) fn contract_with_output(mut self, name: &str, code: &str, output: &str) -> Self {
        self.contracts.push(ContractDef {
            name: name.to_string(),
            code: ContractSource::Tolk(code.to_string()),
            depends: Vec::new(),
            output: Some(output.to_string()),
            dir: None,
        });
        self
    }

    pub(crate) fn test_file(mut self, name: &str, code: &str) -> Self {
        self.tests.push((name.to_string(), code.to_string()));
        self
    }

    /// Load a test file from disk
    ///
    /// # Examples
    /// ```
    /// .test_file_from_path("test", "tests/ffi/get_config.tolk")
    /// ```
    pub(crate) fn test_file_from_path(mut self, name: &str, path: &str) -> Self {
        let code = fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("Failed to read test file from {}: {}", path, e));
        self.tests.push((name.to_string(), code));
        self
    }

    /// Load a contract from a `.tolk` file
    ///
    /// # Examples
    /// ```
    /// .contract_from_path("simple", "contracts/simple.tolk")
    /// ```
    pub(crate) fn contract_from_path(mut self, name: &str, path: &str) -> Self {
        let code = fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("Failed to read contract file from {}: {}", path, e));
        self.contracts.push(ContractDef {
            name: name.to_string(),
            code: ContractSource::Tolk(code),
            depends: Vec::new(),
            output: None,
            dir: None,
        });
        self
    }

    /// Load a custom file from disk
    ///
    /// # Examples
    /// ```
    /// .file_from_path("lib/math", "lib/math.tolk")
    /// ```
    pub(crate) fn file_from_path(mut self, dest_path: &str, src_path: &str) -> Self {
        let code = fs::read_to_string(src_path)
            .unwrap_or_else(|e| panic!("Failed to read file from {}: {}", src_path, e));
        self.files.push((dest_path.to_string(), code));
        self
    }

    /// Add a script file
    ///
    /// # Examples
    /// ```
    /// .script_file("hello", r#"print("Hello");"#)
    /// ```
    pub(crate) fn script_file(mut self, name: &str, code: &str) -> Self {
        self.files
            .push((format!("scripts/{name}"), code.to_string()));
        self
    }

    /// Add a custom file to the project (e.g., library files)
    ///
    /// # Examples
    /// ```
    /// .file("lib/math", "fun add(a: int, b: int): int { return a + b; }")
    /// ```
    pub(crate) fn file(mut self, path: &str, code: &str) -> Self {
        self.files.push((path.to_string(), code.to_string()));
        self
    }

    /// Add a custom raw file to the project (e.g., library files)
    ///
    /// # Examples
    /// ```
    /// .raw_file("foo.hex", "...")
    /// ```
    pub(crate) fn raw_file(mut self, path: &str, code: &str) -> Self {
        self.raw_files.push((path.to_string(), code.to_string()));
        self
    }

    /// Configure test settings in Acton.toml
    ///
    /// # Examples
    /// ```
    /// .with_test_config(TestConfig {
    ///     filter: Some("test-unit-.*".to_string()),
    ///     coverage: Some(true),
    ///     backtrace: Some("full".to_string()),
    /// })
    /// ```
    pub(crate) fn with_test_config(mut self, config: TestConfig) -> Self {
        self.test_config = Some(config);
        self
    }

    pub(crate) fn build(self) -> Project {
        let project_path = self.temp_dir.path().join(&self.name);
        fs::create_dir_all(&project_path).expect("Failed to create project dir");

        Self::copy_lib_to(self.temp_dir.path());

        let contracts_dir = project_path.join("contracts");
        fs::create_dir_all(&contracts_dir).expect("Failed to create contracts dir");

        let tests_dir = project_path.join("tests");
        fs::create_dir_all(&tests_dir).expect("Failed to create tests dir");

        for contract in &self.contracts {
            let contract_dir = if let Some(ref custom_dir) = contract.dir {
                project_path.join(custom_dir)
            } else {
                contracts_dir.clone()
            };
            fs::create_dir_all(&contract_dir).expect("Failed to create contract directory");

            match &contract.code {
                ContractSource::Tolk(code) => {
                    let file_path = contract_dir.join(format!("{}.tolk", contract.name));
                    fs::write(file_path, code).expect("Failed to write contract file");
                }
                ContractSource::Boc(boc_data) => {
                    let file_path = contract_dir.join(format!("{}.boc", contract.name));
                    fs::write(file_path, boc_data).expect("Failed to write BoC file");
                }
            }
        }

        for (name, code) in &self.tests {
            let adjusted_code = Self::adjust_imports(code);
            let file_path = tests_dir.join(format!("{name}.test.tolk"));
            fs::write(file_path, adjusted_code).expect("Failed to write test file");
        }

        for (path, code) in &self.files {
            let file_path = project_path.join(format!("{path}.tolk"));
            if let Some(parent) = file_path.parent() {
                fs::create_dir_all(parent).expect("Failed to create parent directories");
            }
            fs::write(file_path, code).expect("Failed to write custom file");
        }

        for (path, code) in &self.raw_files {
            let file_path = project_path.join(path);
            if let Some(parent) = file_path.parent() {
                fs::create_dir_all(parent).expect("Failed to create parent directories");
            }
            fs::write(file_path, code).expect("Failed to write raw file");
        }

        if self.create_acton_toml {
            Self::create_acton_toml(
                &project_path,
                &self.name,
                &self.contracts,
                &self.scripts,
                &self.mappings,
                &self.lint_levels,
                &self.lint_excludes,
                self.lint_max_warnings,
                self.lint_sarif_path.clone(),
                &self.test_config,
                &self.license,
            );
        }

        Project {
            path: project_path,
            _temp_dir: self.temp_dir,
        }
    }

    fn copy_lib_to(temp_path: &Path) {
        use include_dir::{Dir, include_dir};
        static LIB_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/lib");

        let lib_path = temp_path.join("lib");
        fs::create_dir_all(&lib_path).expect("Failed to create lib dir");
        LIB_DIR
            .extract(&lib_path)
            .expect("Failed to extract lib dir");
    }

    fn adjust_imports(code: &str) -> String {
        code.replace("import \"../../../../lib/", "import \"../../lib/")
    }

    #[allow(clippy::too_many_arguments)]
    fn create_acton_toml(
        project_path: &Path,
        name: &str,
        contracts: &[ContractDef],
        scripts: &[(String, String)],
        mappings: &[(String, String)],
        lint_levels: &BTreeMap<String, String>,
        lint_excludes: &[String],
        lint_max_warnings: Option<usize>,
        lint_sarif_path: Option<String>,
        test_config: &Option<TestConfig>,
        license: &Option<String>,
    ) {
        use std::fmt::Write as _;

        let license_line = if let Some(lic) = license {
            format!("license = \"{lic}\"\n")
        } else {
            String::new()
        };

        let mut toml_content = format!(
            r#"[package]
name = "{name}"
description = "A test project"
version = "0.1.0"
{license_line}
"#
        );

        for contract in contracts {
            let file_extension = match &contract.code {
                ContractSource::Tolk(_) => "tolk",
                ContractSource::Boc(_) => "boc",
            };

            let contract_path = if let Some(ref custom_dir) = contract.dir {
                format!("{}/{}.{}", custom_dir, contract.name, file_extension)
            } else {
                format!("contracts/{}.{}", contract.name, file_extension)
            };

            write!(
                toml_content,
                "[contracts.{}]\nname = \"{}\"\nsrc = \"{}\"\n",
                contract.name.to_lowercase().replace('-', "_"),
                contract.name,
                contract_path,
            )
            .ok();

            // Generate dependencies
            if contract.depends.is_empty() {
                toml_content.push_str("depends = []\n");
            } else {
                let has_detailed = contract
                    .depends
                    .iter()
                    .any(|d| d.kind.is_some() || d.function.is_some() || d.path.is_some());

                if has_detailed {
                    toml_content.push_str("depends = [\n");
                    for dep in &contract.depends {
                        if dep.kind.is_none() && dep.function.is_none() && dep.path.is_none() {
                            toml_content.push_str(&format!("  \"{}\",\n", dep.name));
                        } else {
                            toml_content.push_str(&format!("  {{ name = \"{}\"", dep.name));
                            if let Some(kind) = &dep.kind {
                                toml_content.push_str(&format!(", kind = \"{kind}\""));
                            }
                            if let Some(function) = &dep.function {
                                toml_content.push_str(&format!(", function = \"{function}\""));
                            }
                            if let Some(path) = &dep.path {
                                toml_content.push_str(&format!(", path = \"{path}\""));
                            }
                            toml_content.push_str(" },\n");
                        }
                    }
                    toml_content.push_str("]\n");
                } else {
                    toml_content.push_str(&format!(
                        "depends = [{}]\n",
                        contract
                            .depends
                            .iter()
                            .map(|d| format!("\"{}\"", d.name))
                            .collect::<Vec<_>>()
                            .join(", ")
                    ));
                }
            }

            if let Some(output) = &contract.output {
                toml_content.push_str(&format!("output = \"{output}\"\n"));
            }

            toml_content.push('\n');
        }

        if !mappings.is_empty() {
            toml_content.push_str("[mappings]\n");
            for (prefix, target) in mappings {
                toml_content.push_str(&format!("\"{prefix}\" = \"{target}\"\n"));
            }
            toml_content.push('\n');
        }

        if !scripts.is_empty() {
            toml_content.push_str("[scripts]\n");
            for (name, cmd) in scripts {
                toml_content.push_str(&format!("{name} = \"{cmd}\"\n"));
            }
            toml_content.push('\n');
        }

        if !lint_excludes.is_empty() || lint_max_warnings.is_some() || lint_sarif_path.is_some() {
            toml_content.push_str("[lint]\n");
            if !lint_excludes.is_empty() {
                toml_content.push_str("exclude = [");
                toml_content.push_str(
                    &lint_excludes
                        .iter()
                        .map(|pattern| format!("\"{pattern}\""))
                        .collect::<Vec<_>>()
                        .join(", "),
                );
                toml_content.push_str("]\n");
            }
            if let Some(max_warnings) = lint_max_warnings {
                toml_content.push_str(&format!("max-warnings = {max_warnings}\n"));
            }
            toml_content.push('\n');
        }

        if let Some(path) = lint_sarif_path {
            toml_content.push_str("[lint.output.sarif]\n");
            toml_content.push_str(&format!("path = \"{path}\"\n\n"));
        }

        if !lint_levels.is_empty() {
            toml_content.push_str("[lint.rules]\n");
            for (rule, level) in lint_levels {
                toml_content.push_str(&format!("{rule} = \"{level}\"\n"));
            }
            toml_content.push('\n');
        }

        // Add [test] section if test_config is provided
        if let Some(config) = test_config {
            toml_content.push_str("[test]\n");

            if let Some(filter) = &config.filter {
                toml_content.push_str(&format!("filter = \"{filter}\"\n"));
            }

            if let Some(exclude_patterns) = &config.exclude_patterns {
                toml_content.push_str(&format!(
                    "exclude = [{}]\n",
                    exclude_patterns
                        .iter()
                        .map(|p| format!("\"{p}\""))
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }

            if let Some(include_patterns) = &config.include_patterns {
                toml_content.push_str(&format!(
                    "include = [{}]\n",
                    include_patterns
                        .iter()
                        .map(|p| format!("\"{p}\""))
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }

            if let Some(reporters) = &config.reporters {
                toml_content.push_str(&format!(
                    "reporter = [{}]\n",
                    reporters
                        .iter()
                        .map(|r| format!("\"{r}\""))
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }

            if let Some(debug) = config.debug {
                toml_content.push_str(&format!("debug = {debug}\n"));
            }

            if let Some(debug_port) = config.debug_port {
                toml_content.push_str(&format!("debug-port = {debug_port}\n"));
            }

            if let Some(backtrace) = &config.backtrace {
                toml_content.push_str(&format!("backtrace = \"{backtrace}\"\n"));
            }

            if let Some(coverage) = config.coverage {
                toml_content.push_str(&format!("coverage = {coverage}\n"));
            }

            if let Some(coverage_format) = &config.coverage_format {
                toml_content.push_str(&format!("coverage-format = \"{coverage_format}\"\n"));
            }

            if let Some(coverage_file) = &config.coverage_file {
                toml_content.push_str(&format!("coverage-file = \"{coverage_file}\"\n"));
            }

            if let Some(junit_path) = &config.junit_path {
                toml_content.push_str(&format!("junit-path = \"{junit_path}\"\n"));
            }

            if let Some(junit_merge) = config.junit_merge {
                toml_content.push_str(&format!("junit-merge = {junit_merge}\n"));
            }

            if let Some(fail_fast) = config.fail_fast {
                toml_content.push_str(&format!("fail-fast = {fail_fast}\n"));
            }

            toml_content.push('\n');
        }

        let config_path = project_path.join("Acton.toml");
        fs::write(config_path, toml_content).expect("Failed to write Acton.toml");
    }
}

pub(crate) struct Project {
    path: PathBuf,
    _temp_dir: TempDir,
}

impl Project {
    #[allow(dead_code)]
    pub(crate) fn acton(&self) -> ActonCommand {
        let cmd = snapbox::cmd::Command::new(acton_exe()).with_assert(assert_ui());
        ActonCommand {
            cmd,
            project: Arc::new(ProjectRef {
                path: self.path.clone(),
            }),
            test_path: None,
            filter: None,
            build_contract: None,
            build_clear_cache: false,
            build_graph: None,
            build_out_dir: None,
            build_output_fift: None,
            disasm_string: None,
            disasm_output: None,
            disasm_address: None,
            disasm_api_key: None,
            disasm_net: None,
            disasm_follow_libraries: false,
            compile_json: false,
            compile_base64_only: false,
            compile_boc: None,
            compile_fift: None,
            compile_source_map: None,
            test_reporters: Vec::new(),
            junit_merge: false,
            test_exclude_patterns: Vec::new(),
            test_include_patterns: Vec::new(),
            verify_contract: None,
            verify_address: None,
            verify_wallet: None,
            verify_network: None,
            script_broadcast: false,
            test_fail_fast: false,
            script_fork_net: None,
            build_info: false,
            force_no_color_env: true,
            color_mode: None,
        }
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }
}

pub(crate) struct ProjectRef {
    pub path: PathBuf,
}

pub(crate) struct ActonCommand {
    pub(crate) cmd: snapbox::cmd::Command,
    pub(crate) project: Arc<ProjectRef>,
    pub(crate) test_path: Option<String>,
    pub(crate) filter: Option<String>,
    pub(crate) build_contract: Option<String>,
    pub(crate) build_clear_cache: bool,
    pub(crate) build_graph: Option<Option<String>>,
    pub(crate) build_out_dir: Option<String>,
    pub(crate) build_output_fift: Option<String>,
    pub(crate) disasm_string: Option<String>,
    pub(crate) disasm_output: Option<String>,
    pub(crate) disasm_address: Option<String>,
    pub(crate) disasm_api_key: Option<String>,
    pub(crate) disasm_net: Option<String>,
    pub(crate) disasm_follow_libraries: bool,
    pub(crate) compile_json: bool,
    pub(crate) compile_base64_only: bool,
    pub(crate) compile_boc: Option<String>,
    pub(crate) compile_fift: Option<String>,
    pub(crate) compile_source_map: Option<String>,
    pub(crate) test_reporters: Vec<String>,
    pub(crate) junit_merge: bool,
    pub(crate) test_exclude_patterns: Vec<String>,
    pub(crate) test_include_patterns: Vec<String>,
    pub(crate) verify_contract: Option<String>,
    pub(crate) verify_address: Option<String>,
    pub(crate) verify_wallet: Option<String>,
    pub(crate) verify_network: Option<String>,
    pub(crate) script_broadcast: bool,
    pub(crate) test_fail_fast: bool,
    pub(crate) script_fork_net: Option<String>,
    pub(crate) build_info: bool,
    pub(crate) force_no_color_env: bool,
    pub(crate) color_mode: Option<ColorMode>,
}

#[allow(dead_code)]
impl ActonCommand {
    pub(crate) fn build(mut self) -> Self {
        self.cmd = self.cmd.arg("build").current_dir(&self.project.path);
        self
    }

    /// Start test command (defaults to running all tests in current directory)
    pub(crate) fn test(mut self) -> Self {
        self.cmd = self.cmd.arg("test").current_dir(&self.project.path);
        self
    }

    /// Start init command
    pub(crate) fn init(mut self) -> Self {
        self.cmd = self.cmd.arg("init").current_dir(&self.project.path);
        self
    }

    /// Start wrapper command
    pub(crate) fn wrapper(mut self, contract_id: &str) -> Self {
        self.cmd = self
            .cmd
            .arg("wrapper")
            .arg(contract_id)
            .current_dir(&self.project.path);
        self
    }

    pub(crate) fn storage_struct(mut self, name: &str) -> Self {
        self.cmd = self.cmd.arg("--storage-struct").arg(name);
        self
    }

    pub(crate) fn generate_test_stub(mut self) -> Self {
        self.cmd = self.cmd.arg("--test");
        self
    }

    /// Specify output wrapper file
    pub(crate) fn wrapper_output(mut self, path: &str) -> Self {
        self.cmd = self.cmd.arg("--output").arg(path);
        self
    }

    /// Specify output test file
    pub(crate) fn test_output(mut self, path: &str) -> Self {
        self.cmd = self.cmd.arg("--test-output").arg(path);
        self
    }

    /// Start script command
    ///
    /// # Examples
    /// ```
    /// .script("scripts/hello.tolk")
    /// ```
    pub(crate) fn script(mut self, script_path: &str) -> Self {
        self.cmd = self
            .cmd
            .arg("script")
            .arg(script_path)
            .current_dir(&self.project.path);
        self
    }

    pub(crate) fn run_script_cmd(mut self, script_name: &str) -> Self {
        self.cmd = self
            .cmd
            .arg("run")
            .arg(script_name)
            .current_dir(&self.project.path);
        self
    }

    pub(crate) fn arg(mut self, arg: &str) -> Self {
        self.cmd = self.cmd.arg(arg);
        self
    }

    pub(crate) fn env(mut self, key: &str, value: &str) -> Self {
        self.cmd = self.cmd.env(key, value);
        self
    }

    pub(crate) fn env_remove(mut self, key: &str) -> Self {
        self.cmd = self.cmd.env_remove(key);
        self
    }

    /// Keep process color-related environment variables unchanged.
    ///
    /// By default, tests force `NO_COLOR=1` for stable snapshots.
    /// Use this when you need to validate auto color detection behavior.
    pub(crate) fn keep_color_env(mut self) -> Self {
        self.force_no_color_env = false;
        self
    }

    pub(crate) fn color_mode(mut self, mode: ColorMode) -> Self {
        self.color_mode = Some(mode);
        self
    }

    pub(crate) fn current_dir(mut self, path: impl AsRef<Path>) -> Self {
        self.cmd = self.cmd.current_dir(path);
        self
    }

    /// Start disasm command (without input - use with `disasm_file` or `disasm_string`)
    ///
    /// # Examples
    /// ```
    /// .disasm().disasm_file("contract.boc")
    /// .disasm().disasm_string("hex_or_base64_string")
    /// ```
    pub(crate) fn disasm(mut self) -> Self {
        self.cmd = self.cmd.arg("disasm").current_dir(&self.project.path);
        self
    }

    pub(crate) fn fmt(mut self) -> Self {
        self.cmd = self.cmd.arg("fmt").current_dir(&self.project.path);
        self
    }

    pub(crate) fn check(mut self) -> Self {
        self.cmd = self.cmd.arg("check").current_dir(&self.project.path);
        self
    }

    /// Start disasm command with file input
    ///
    /// # Examples
    /// ```
    /// .disasm_file("contract.boc")
    /// ```
    pub(crate) fn disasm_file(mut self, file_path: &str) -> Self {
        self.cmd = self
            .cmd
            .arg("disasm")
            .arg(file_path)
            .current_dir(&self.project.path);
        self
    }

    /// Start disasm command with string input (hex or base64)
    ///
    /// # Examples
    /// ```
    /// .disasm_string("base64_encoded_boc")
    /// ```
    pub(crate) fn disasm_string(mut self, boc_string: &str) -> Self {
        self.cmd = self.cmd.current_dir(&self.project.path);
        self.disasm_string = Some(boc_string.to_string());
        self
    }

    /// Specify output file for disasm result
    ///
    /// # Examples
    /// ```
    /// .disasm_file("contract.boc").with_output("output.tasm")
    /// ```
    pub(crate) fn with_output(mut self, output_path: &str) -> Self {
        self.disasm_output = Some(output_path.to_string());
        self
    }

    /// Specify contract address for blockchain disasm
    ///
    /// # Examples
    /// ```
    /// .disasm().with_address("UQA_ftKIJsHEAE_UgtFOUK15hPzycZooFuUr8duyY9T3kwwM")
    /// ```
    pub(crate) fn with_address(mut self, address: &str) -> Self {
        self.disasm_address = Some(address.to_string());
        self
    }

    /// Specify API key for `TonCenter` requests
    ///
    /// # Examples
    /// ```
    /// .disasm().with_address("...").with_api_key("your-api-key")
    /// ```
    pub(crate) fn with_api_key(mut self, api_key: &str) -> Self {
        self.disasm_api_key = Some(api_key.to_string());
        self
    }

    /// Specify network for library fetching (testnet or mainnet)
    ///
    /// # Examples
    /// ```
    /// .disasm().with_address("...").with_net("mainnet")
    /// ```
    pub(crate) fn with_net(mut self, net: &str) -> Self {
        self.disasm_net = Some(net.to_string());
        self
    }

    /// Enable following library references
    ///
    /// # Examples
    /// ```
    /// .disasm().with_address("...").follow_libraries()
    /// ```
    pub(crate) fn follow_libraries(mut self) -> Self {
        self.disasm_follow_libraries = true;
        self
    }

    pub(crate) fn compile(mut self, file_path: &str) -> Self {
        self.cmd = self
            .cmd
            .arg("compile")
            .arg(file_path)
            .current_dir(&self.project.path);
        self
    }

    pub(crate) fn with_json(mut self) -> Self {
        self.compile_json = true;
        self
    }

    pub(crate) fn base64_only(mut self) -> Self {
        self.compile_base64_only = true;
        self
    }

    pub(crate) fn with_boc_output(mut self, boc_path: &str) -> Self {
        self.compile_boc = Some(boc_path.to_string());
        self
    }

    pub(crate) fn with_fift_output(mut self, fift_path: &str) -> Self {
        self.compile_fift = Some(fift_path.to_string());
        self
    }

    pub(crate) fn with_source_map(mut self, source_map_path: &str) -> Self {
        self.compile_source_map = Some(source_map_path.to_string());
        self
    }

    /// Specify path to test file or directory
    ///
    /// # Examples
    /// ```
    /// .test().path(".")                   // All tests (default)
    /// .test().path("tests/my.test.tolk")  // Specific file
    /// .test().path("tests/")              // Specific directory
    /// ```
    pub(crate) fn path(mut self, path: &str) -> Self {
        self.test_path = Some(path.to_string());
        self
    }

    /// Filter tests by name pattern (regex)
    ///
    /// # Examples
    /// ```
    /// .test().filter("test-basic")        // Run tests matching "test-basic"
    /// .test().filter("counter.*")         // Run tests starting with "counter"
    /// ```
    pub(crate) fn filter(mut self, pattern: &str) -> Self {
        self.filter = Some(pattern.to_string());
        self
    }

    pub(crate) fn exclude_pattern(mut self, pattern: &str) -> Self {
        self.test_exclude_patterns.push(pattern.to_string());
        self
    }

    pub(crate) fn include_pattern(mut self, pattern: &str) -> Self {
        self.test_include_patterns.push(pattern.to_string());
        self
    }

    /// Enable backtrace output
    ///
    /// # Examples
    /// ```
    /// .test().with_backtrace("full")      // Full backtrace
    /// ```
    pub(crate) fn with_backtrace(mut self, level: &str) -> Self {
        self.cmd = self.cmd.arg("--backtrace").arg(level);
        self
    }

    /// Enable coverage collection
    pub(crate) fn with_coverage(mut self) -> Self {
        self.cmd = self.cmd.arg("--coverage");
        self
    }

    /// Enable coverage with specific format (e.g., "lcov")
    pub(crate) fn with_coverage_format(mut self, format: &str) -> Self {
        self.cmd = self.cmd.arg("--coverage-format").arg(format);
        self
    }

    /// Enable coverage with custom output file
    pub(crate) fn with_coverage_file(mut self, file: &str) -> Self {
        self.cmd = self.cmd.arg("--coverage-file").arg(file);
        self
    }

    /// Add test reporter
    ///
    /// # Examples
    /// ```
    /// .test().with_reporter("teamcity")           // TeamCity format
    /// .test().with_reporter("junit")              // JUnit XML format
    /// .test().with_reporter("console")            // Console format (default)
    /// ```
    pub(crate) fn with_reporter(mut self, reporter: &str) -> Self {
        self.test_reporters.push(reporter.to_string());
        self
    }

    /// Enable fail-fast mode
    pub(crate) fn fail_fast(mut self) -> Self {
        self.test_fail_fast = true;
        self
    }

    /// Enable `JUnit` merge mode (all suites in single file)
    ///
    /// # Examples
    /// ```
    /// .test().with_reporter("junit").with_junit_merge()
    /// ```
    pub(crate) fn with_junit_merge(mut self) -> Self {
        self.junit_merge = true;
        self
    }

    /// Build specific contract (only for build command)
    ///
    /// # Examples
    /// ```
    /// .build().contract("my_contract")   // Build only my_contract and its dependencies
    /// ```
    pub(crate) fn contract(mut self, name: &str) -> Self {
        self.build_contract = Some(name.to_string());
        self
    }

    pub(crate) fn verify(mut self) -> Self {
        self.cmd = self.cmd.arg("verify").current_dir(&self.project.path);
        self
    }

    pub(crate) fn verify_contract(mut self, name: &str) -> Self {
        self.verify_contract = Some(name.to_string());
        self
    }

    pub(crate) fn verify_address(mut self, address: &str) -> Self {
        self.verify_address = Some(address.to_string());
        self
    }

    pub(crate) fn wallet(mut self, wallet: &str) -> Self {
        self.verify_wallet = Some(wallet.to_string());
        self
    }

    pub(crate) fn verify_network(mut self, network: &str) -> Self {
        self.verify_network = Some(network.to_string());
        self
    }

    pub(crate) fn fork_net(mut self, network: &str) -> Self {
        self.script_fork_net = Some(network.to_string());
        self
    }

    pub(crate) fn library(mut self) -> Self {
        self.cmd = self.cmd.arg("library").current_dir(&self.project.path);
        self
    }

    pub(crate) fn fetch(mut self, hash: &str) -> Self {
        self.cmd = self.cmd.arg("fetch").arg(hash);
        self
    }

    pub(crate) fn publish(mut self) -> Self {
        self.cmd = self.cmd.arg("publish");
        self
    }

    pub(crate) fn with_code(mut self, code: &str) -> Self {
        self.cmd = self.cmd.arg("--code").arg(code);
        self
    }

    pub(crate) fn wallet_new(mut self) -> Self {
        self.cmd = self
            .cmd
            .arg("wallet")
            .arg("new")
            .arg("--secure")
            .arg("false")
            .current_dir(&self.project.path);
        self
    }

    pub(crate) fn wallet_import(mut self) -> Self {
        self.cmd = self
            .cmd
            .arg("wallet")
            .arg("import")
            .arg("--secure")
            .arg("false")
            .current_dir(&self.project.path);
        self
    }

    pub(crate) fn wallet_list(mut self) -> Self {
        self.cmd = self
            .cmd
            .arg("wallet")
            .arg("list")
            .current_dir(&self.project.path);
        self
    }

    pub(crate) fn wallet_get(mut self) -> Self {
        self.cmd = self
            .cmd
            .arg("wallet")
            .arg("get")
            .current_dir(&self.project.path);
        self
    }

    pub(crate) fn wallet_remove(mut self) -> Self {
        self.cmd = self
            .cmd
            .arg("wallet")
            .arg("remove")
            .current_dir(&self.project.path);
        self
    }

    pub(crate) fn with_duration(mut self, duration: &str) -> Self {
        self.cmd = self.cmd.arg("--duration").arg(duration);
        self
    }

    pub(crate) fn with_disasm_flag(mut self) -> Self {
        self.cmd = self.cmd.arg("--disasm");
        self
    }

    /// Clear compilation cache (for build and script commands)
    ///
    /// # Examples
    /// ```
    /// .build().clear_cache()              // Clear cache before building
    /// .script("test.tolk").clear_cache()  // Clear cache before running script
    /// ```
    pub(crate) fn clear_cache(mut self) -> Self {
        self.build_clear_cache = true;
        self
    }

    /// Generate dependency graph SVG (only for build command)
    ///
    /// # Examples
    /// ```
    /// .build().with_graph(None)           // Generate deps.svg (default)
    /// .build().with_graph(Some("my.svg")) // Generate my.svg
    /// ```
    pub(crate) fn with_graph(mut self, path: Option<&str>) -> Self {
        self.build_graph = Some(path.map(ToString::to_string));
        self
    }

    /// Set output directory for build artifacts (only for build command)
    ///
    /// # Examples
    /// ```
    /// .build().with_out_dir("artifacts")     // Use artifacts/ directory
    /// .build().with_out_dir("dist/build")    // Use dist/build/ directory
    /// ```
    pub(crate) fn with_out_dir(mut self, path: &str) -> Self {
        self.build_out_dir = Some(path.to_string());
        self
    }

    /// Set output directory for compiled Fift files (only for build command)
    pub(crate) fn with_output_fift(mut self, path: &str) -> Self {
        self.build_output_fift = Some(path.to_string());
        self
    }

    /// Enable info output for build command
    pub(crate) fn with_info(mut self) -> Self {
        self.build_info = true;
        self
    }

    /// Enable broadcast mode for script execution (only for script command)
    ///
    /// # Examples
    /// ```
    /// .script("deploy.tolk").broadcast()     // Send transactions to blockchain
    /// ```
    pub(crate) fn broadcast(mut self) -> Self {
        self.script_broadcast = true;
        self
    }

    /// Run the command and return output
    pub(crate) fn run(mut self) -> TestOutput {
        if let Some(path) = self.test_path {
            self.cmd = self.cmd.arg(path);
        }

        if let Some(filter) = self.filter {
            self.cmd = self.cmd.arg("--filter").arg(filter);
        }

        if !self.test_reporters.is_empty() {
            for reporter in &self.test_reporters {
                self.cmd = self.cmd.arg("--reporter").arg(reporter);
            }
        }

        if self.junit_merge {
            self.cmd = self.cmd.arg("--junit-merge");
        }

        for pattern in &self.test_exclude_patterns {
            self.cmd = self.cmd.arg("--exclude").arg(pattern);
        }

        for pattern in &self.test_include_patterns {
            self.cmd = self.cmd.arg("--include").arg(pattern);
        }

        if self.test_fail_fast {
            self.cmd = self.cmd.arg("--fail-fast");
        }

        if let Some(contract) = self.build_contract {
            self.cmd = self.cmd.arg(contract);
        }

        if let Some(contract) = self.verify_contract {
            self.cmd = self.cmd.arg(contract);
        }

        if let Some(address) = self.verify_address {
            self.cmd = self.cmd.arg("--address").arg(address);
        }

        if let Some(wallet) = self.verify_wallet {
            self.cmd = self.cmd.arg("--wallet").arg(wallet);
        }

        if let Some(network) = self.verify_network {
            self.cmd = self.cmd.arg("--net").arg(network);
        }

        if let Some(network) = self.script_fork_net {
            self.cmd = self.cmd.arg("--fork-net").arg(network);
        }

        if self.build_clear_cache {
            self.cmd = self.cmd.arg("--clear-cache");
        }

        if self.script_broadcast {
            self.cmd = self.cmd.arg("--broadcast");
        }

        if let Some(graph_path) = self.build_graph {
            self.cmd = self.cmd.arg("--graph");
            if let Some(path) = graph_path {
                self.cmd = self.cmd.arg(path);
            } else {
                self.cmd = self.cmd.arg("");
            }
        }

        if let Some(out_dir) = self.build_out_dir {
            self.cmd = self.cmd.arg("--out-dir").arg(out_dir);
        }

        if let Some(output_fift_dir) = self.build_output_fift {
            self.cmd = self.cmd.arg("--output-fift").arg(output_fift_dir);
        }

        if self.build_info {
            self.cmd = self.cmd.arg("--info");
        }

        if let Some(boc_string) = self.disasm_string {
            self.cmd = self.cmd.arg("--string").arg(boc_string);
        }

        if let Some(output_file) = self.disasm_output {
            self.cmd = self.cmd.arg("--output").arg(output_file);
        }

        if let Some(address) = self.disasm_address {
            self.cmd = self.cmd.arg("--address").arg(address);
        }

        if let Some(api_key) = self.disasm_api_key {
            self.cmd = self.cmd.arg("--api-key").arg(api_key);
        }

        if let Some(net) = self.disasm_net {
            self.cmd = self.cmd.arg("--net").arg(net);
        }

        if self.disasm_follow_libraries {
            self.cmd = self.cmd.arg("--follow-libraries");
        }

        if self.compile_json {
            self.cmd = self.cmd.arg("--json");
        }

        if self.compile_base64_only {
            self.cmd = self.cmd.arg("--base64-only");
        }

        if let Some(boc_path) = self.compile_boc {
            self.cmd = self.cmd.arg("--boc").arg(boc_path);
        }

        if let Some(fift_path) = self.compile_fift {
            self.cmd = self.cmd.arg("--fift").arg(fift_path);
        }

        if let Some(source_map_path) = self.compile_source_map {
            self.cmd = self.cmd.arg("--source-map").arg(source_map_path);
        }

        if let Some(mode) = self.color_mode {
            self.cmd = self.cmd.arg("--color").arg(mode.to_string());
        }

        if self.force_no_color_env {
            self.cmd = self.cmd.env("NO_COLOR", "1");
        }
        let output = self.cmd.assert();
        TestOutput {
            output,
            project_path: self.project.path.clone(),
        }
    }
}
