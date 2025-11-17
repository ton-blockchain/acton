use crate::common::{acton_exe, assert_ui};
use crate::support::assertions::TestOutput;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tempfile::TempDir;

pub struct ProjectBuilder {
    name: String,
    temp_dir: TempDir,
    contracts: Vec<(String, String)>,
    tests: Vec<(String, String)>,
    files: Vec<(String, String)>,
    test_config: Option<TestConfig>,
}

#[derive(Clone)]
pub struct TestConfig {
    pub filter: Option<String>,
    pub coverage: Option<bool>,
    pub backtrace: Option<String>,
}

impl ProjectBuilder {
    pub fn new(name: &str) -> Self {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        Self {
            name: name.to_string(),
            temp_dir,
            contracts: Vec::new(),
            tests: Vec::new(),
            files: Vec::new(),
            test_config: None,
        }
    }

    pub fn contract(mut self, name: &str, code: &str) -> Self {
        self.contracts.push((name.to_string(), code.to_string()));
        self
    }

    pub fn test_file(mut self, name: &str, code: &str) -> Self {
        self.tests.push((name.to_string(), code.to_string()));
        self
    }

    /// Add a custom file to the project (e.g., library files)
    ///
    /// # Examples
    /// ```
    /// .file("lib/math", "fun add(a: int, b: int): int { return a + b; }")
    /// ```
    pub fn file(mut self, path: &str, code: &str) -> Self {
        self.files.push((path.to_string(), code.to_string()));
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
    pub fn with_test_config(mut self, config: TestConfig) -> Self {
        self.test_config = Some(config);
        self
    }

    pub fn build(self) -> Project {
        let project_path = self.temp_dir.path().join(&self.name);
        fs::create_dir_all(&project_path).expect("Failed to create project dir");

        Self::copy_lib_to(&self.temp_dir.path());

        let contracts_dir = project_path.join("contracts");
        fs::create_dir_all(&contracts_dir).expect("Failed to create contracts dir");

        let tests_dir = project_path.join("tests");
        fs::create_dir_all(&tests_dir).expect("Failed to create tests dir");

        for (name, code) in &self.contracts {
            let file_path = contracts_dir.join(format!("{}.tolk", name));
            fs::write(file_path, code).expect("Failed to write contract file");
        }

        for (name, code) in &self.tests {
            let adjusted_code = Self::adjust_imports(code);
            let file_path = tests_dir.join(format!("{}_test.tolk", name));
            fs::write(file_path, adjusted_code).expect("Failed to write test file");
        }

        for (path, code) in &self.files {
            let file_path = project_path.join(format!("{}.tolk", path));
            if let Some(parent) = file_path.parent() {
                fs::create_dir_all(parent).expect("Failed to create parent directories");
            }
            fs::write(file_path, code).expect("Failed to write custom file");
        }

        Self::create_acton_toml(
            &project_path,
            &self.name,
            &self.contracts,
            &self.test_config,
        );

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

    fn create_acton_toml(
        project_path: &Path,
        name: &str,
        contracts: &[(String, String)],
        test_config: &Option<TestConfig>,
    ) {
        let mut toml_content = format!(
            r#"[package]
name = "{}"
description = "A test project"
version = "0.1.0"
license = "MIT"

"#,
            name
        );

        for (contract_name, _) in contracts {
            toml_content.push_str(&format!(
                r#"[contracts.{}]
name = "{}"
src = "contracts/{}.tolk"
depends = []

"#,
                contract_name.to_lowercase(),
                contract_name,
                contract_name
            ));
        }

        // Add [test] section if test_config is provided
        if let Some(config) = test_config {
            toml_content.push_str("[test]\n");

            if let Some(filter) = &config.filter {
                toml_content.push_str(&format!("filter = \"{}\"\n", filter));
            }

            if let Some(coverage) = config.coverage {
                toml_content.push_str(&format!("coverage = {}\n", coverage));
            }

            if let Some(backtrace) = &config.backtrace {
                toml_content.push_str(&format!("backtrace = \"{}\"\n", backtrace));
            }

            toml_content.push_str("\n");
        }

        let config_path = project_path.join("Acton.toml");
        fs::write(config_path, toml_content).expect("Failed to write Acton.toml");
    }
}

pub struct Project {
    path: PathBuf,
    _temp_dir: TempDir,
}

impl Project {
    pub fn acton(&self) -> ActonCommand {
        let cmd = snapbox::cmd::Command::new(acton_exe()).with_assert(assert_ui());
        ActonCommand {
            cmd,
            project: Arc::new(ProjectRef {
                path: self.path.clone(),
            }),
            test_path: None,
            filter: None,
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

pub struct ProjectRef {
    pub path: PathBuf,
}

pub struct ActonCommand {
    pub(crate) cmd: snapbox::cmd::Command,
    pub(crate) project: Arc<ProjectRef>,
    pub(crate) test_path: Option<String>,
    pub(crate) filter: Option<String>,
}

impl ActonCommand {
    /// Start test command (defaults to running all tests in current directory)
    pub fn test(mut self) -> Self {
        self.cmd = self.cmd.arg("test").current_dir(&self.project.path);
        self
    }

    /// Specify path to test file or directory
    ///
    /// # Examples
    /// ```
    /// .test().path(".")                   // All tests (default)
    /// .test().path("tests/my_test.tolk")  // Specific file
    /// .test().path("tests/")              // Specific directory
    /// ```
    pub fn path(mut self, path: &str) -> Self {
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
    pub fn filter(mut self, pattern: &str) -> Self {
        self.filter = Some(pattern.to_string());
        self
    }

    /// Enable backtrace output
    ///
    /// # Examples
    /// ```
    /// .test().with_backtrace("full")      // Full backtrace
    /// ```
    pub fn with_backtrace(mut self, level: &str) -> Self {
        self.cmd = self.cmd.arg("--backtrace").arg(level);
        self
    }

    /// Enable coverage collection
    pub fn with_coverage(mut self) -> Self {
        self.cmd = self.cmd.arg("--coverage");
        self
    }

    /// Enable coverage with specific format (e.g., "lcov")
    pub fn with_coverage_format(mut self, format: &str) -> Self {
        self.cmd = self.cmd.arg("--coverage").arg("--format").arg(format);
        self
    }

    /// Run the command and return output
    pub fn run(mut self) -> TestOutput {
        // Add path argument (default to "." if not specified)
        let path = self.test_path.unwrap_or_else(|| ".".to_string());
        self.cmd = self.cmd.arg(path);

        // Add filter if specified
        if let Some(filter) = self.filter {
            self.cmd = self.cmd.arg("--filter").arg(filter);
        }

        self.cmd = self.cmd.env("NO_COLOR", "1");
        let output = self.cmd.assert();
        TestOutput {
            output,
            project_path: self.project.path.clone(),
        }
    }
}
