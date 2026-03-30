use crate::common::{assertion, strip_ansi};
use crate::support::snapshots::{
    normalize_output, normalize_output_keep_ansi, normalize_output_preserve_escapes,
};
use snapbox::Data;
use snapbox::cmd::OutputAssert;
use std::path::{Path, PathBuf};

#[allow(dead_code)]
pub(crate) struct TestOutput {
    pub output: OutputAssert,
    pub project_path: PathBuf,
}

#[allow(dead_code)]
impl TestOutput {
    pub(crate) fn success(self) -> TestSuccess {
        let output = self.output.success();
        TestSuccess {
            output,
            project_path: self.project_path,
        }
    }

    pub(crate) fn failure(self) -> TestFailure {
        let output = self.output.failure();
        TestFailure {
            output,
            project_path: self.project_path,
        }
    }

    /// Assert specific exit code
    pub(crate) fn code(self, expected_code: i32) -> TestSuccess {
        let output = self.output.code(expected_code);
        TestSuccess {
            output,
            project_path: self.project_path,
        }
    }
}

#[allow(dead_code)]
pub(crate) struct TestSuccess {
    output: OutputAssert,
    project_path: PathBuf,
}

#[allow(dead_code)]
pub(crate) struct TestFailure {
    output: OutputAssert,
    project_path: PathBuf,
}

#[allow(dead_code)]
pub(crate) trait TestOutputExt {
    fn assert_passed(&self, count: usize) -> &Self;
    fn assert_failed(&self, count: usize) -> &Self;
    fn assert_skipped(&self, count: usize) -> &Self;
    fn assert_todo(&self, count: usize) -> &Self;
    #[allow(dead_code)]
    fn assert_test_passed(&self, name: &str) -> &Self;
    #[allow(dead_code)]
    fn assert_test_failed(&self, name: &str) -> &Self;
    fn assert_contains(&self, text: &str) -> &Self;
    fn assert_not_contains(&self, text: &str) -> &Self;
    fn assert_stderr_contains(&self, text: &str) -> &Self;
    fn get_stdout(&self) -> String;
    fn get_stderr(&self) -> String;
    fn get_normalized_stdout(&self) -> String;
    fn get_normalized_stdout_keep_ansi(&self) -> String;
    fn get_normalized_stderr(&self) -> String;
    fn assert_snapshot_matches(&self, path: &str) -> &Self;
    fn assert_stdout_svg_snapshot_matches(&self, path: &str) -> &Self;
    fn assert_stderr_svg_snapshot_matches(&self, path: &str) -> &Self;
    fn assert_stderr_snapshot_matches(&self, path: &str) -> &Self;
    fn assert_file_exists(&self, path: &str) -> &Self;
    fn assert_file_contains(&self, path: &str, content: &str) -> &Self;
    fn assert_file_snapshot_matches(&self, file_path: &str, snapshot_path: &str) -> &Self;
}

#[allow(dead_code)]
fn is_json_like_snapshot_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| matches!(ext, "json" | "sarif"))
}

#[allow(dead_code)]
fn preserves_json_field_order(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| matches!(name, "package.json" | "package-lock.json"))
}

#[allow(dead_code)]
fn normalize_file_snapshot_content(
    file_content: &str,
    file_path: &Path,
    project_path: &Path,
) -> String {
    if is_json_like_snapshot_file(file_path) && !preserves_json_field_order(file_path) {
        normalize_output_preserve_escapes(file_content, project_path)
    } else {
        normalize_output(file_content, project_path)
    }
}

#[allow(dead_code)]
fn snapshot_assert_for_file(file_path: &Path) -> snapbox::Assert {
    if is_json_like_snapshot_file(file_path) && !preserves_json_field_order(file_path) {
        assertion().normalize_paths(false)
    } else {
        assertion()
    }
}

impl TestOutputExt for TestSuccess {
    fn assert_passed(&self, count: usize) -> &Self {
        let stdout = strip_ansi(&self.get_stdout());
        let passed_text = format!("✓ {count} passed");
        assert!(
            stdout.contains(&passed_text),
            "Expected '{passed_text}' in stdout, but got:\n{stdout}"
        );
        self
    }

    fn assert_failed(&self, count: usize) -> &Self {
        let stdout = strip_ansi(&self.get_stdout());
        let failed_text = format!("✗ {count} failed");
        assert!(
            stdout.contains(&failed_text),
            "Expected '{failed_text}' in stdout, but got:\n{stdout}"
        );
        self
    }

    fn assert_skipped(&self, count: usize) -> &Self {
        let stdout = strip_ansi(&self.get_stdout());
        let skipped_text = format!("○ {count} skipped");
        assert!(
            stdout.contains(&skipped_text),
            "Expected '{skipped_text}' in stdout, but got:\n{stdout}"
        );
        self
    }

    fn assert_todo(&self, count: usize) -> &Self {
        let stdout = strip_ansi(&self.get_stdout());
        let skipped_text = format!("□ {count} todo");
        assert!(
            stdout.contains(&skipped_text),
            "Expected '{skipped_text}' in stdout, but got:\n{stdout}"
        );
        self
    }

    fn assert_test_passed(&self, name: &str) -> &Self {
        let stdout = strip_ansi(&self.get_stdout());
        let test_line = format!("✓ {name}");
        assert!(
            stdout.contains(&test_line),
            "Expected test '{name}' to pass, but got:\n{stdout}"
        );
        self
    }

    fn assert_test_failed(&self, name: &str) -> &Self {
        let stdout = strip_ansi(&self.get_stdout());
        let test_line = format!("✗ {name}");
        assert!(
            stdout.contains(&test_line),
            "Expected test '{name}' to fail, but got:\n{stdout}"
        );
        self
    }

    fn assert_contains(&self, text: &str) -> &Self {
        let stdout = strip_ansi(&self.get_stdout());
        assert!(
            stdout.contains(text),
            "Expected '{text}' in stdout, but got:\n{stdout}"
        );
        self
    }

    fn assert_not_contains(&self, text: &str) -> &Self {
        let stdout = strip_ansi(&self.get_stdout());
        assert!(
            !stdout.contains(text),
            "Did not expect '{text}' in stdout, but got:\n{stdout}"
        );
        self
    }

    fn assert_stderr_contains(&self, text: &str) -> &Self {
        let stderr = strip_ansi(&self.get_stderr());
        assert!(
            stderr.contains(text),
            "Expected '{text}' in stderr, but got:\n{stderr}"
        );
        self
    }

    fn get_stdout(&self) -> String {
        String::from_utf8(self.output.get_output().stdout.clone())
            .expect("Failed to convert stdout to string")
    }

    fn get_stderr(&self) -> String {
        String::from_utf8(self.output.get_output().stderr.clone())
            .expect("Failed to convert stderr to string")
    }

    fn get_normalized_stdout(&self) -> String {
        normalize_output(&self.get_stdout(), &self.project_path)
    }

    fn get_normalized_stdout_keep_ansi(&self) -> String {
        normalize_output_keep_ansi(&self.get_stdout(), &self.project_path)
    }

    fn get_normalized_stderr(&self) -> String {
        normalize_output(&self.get_stderr(), &self.project_path)
    }

    fn assert_snapshot_matches(&self, path: &str) -> &Self {
        let normalized = self.get_normalized_stdout();
        let assertion = assertion();

        let mut snapshot_path = std::env::current_dir().expect("Failed to get current dir");
        snapshot_path.push("tests");
        snapshot_path.push(path);

        let expected = Data::read_from(&snapshot_path, None);
        assertion.eq(normalized, expected);
        self
    }

    fn assert_stdout_svg_snapshot_matches(&self, path: &str) -> &Self {
        let normalized = self.get_normalized_stdout_keep_ansi();
        let assertion = assertion();

        let mut snapshot_path = std::env::current_dir().expect("Failed to get current dir");
        snapshot_path.push("tests");
        snapshot_path.push(path);

        let expected = Data::read_from(&snapshot_path, Some(snapbox::data::DataFormat::TermSvg));

        assertion.eq(
            Data::from(normalized).coerce_to(snapbox::data::DataFormat::TermSvg),
            expected,
        );
        self
    }

    fn assert_stderr_svg_snapshot_matches(&self, path: &str) -> &Self {
        let normalized = normalize_output_keep_ansi(&self.get_stderr(), &self.project_path);
        let assertion = assertion();

        let mut snapshot_path = std::env::current_dir().expect("Failed to get current dir");
        snapshot_path.push("tests");
        snapshot_path.push(path);

        let expected = Data::read_from(&snapshot_path, Some(snapbox::data::DataFormat::TermSvg));

        assertion.eq(
            Data::from(normalized).coerce_to(snapbox::data::DataFormat::TermSvg),
            expected,
        );
        self
    }

    fn assert_stderr_snapshot_matches(&self, path: &str) -> &Self {
        let normalized = self.get_normalized_stderr();
        let assertion = assertion();

        let mut snapshot_path = std::env::current_dir().expect("Failed to get current dir");
        snapshot_path.push("tests");
        snapshot_path.push(path);

        let expected = Data::read_from(&snapshot_path, None);
        assertion.eq(normalized, expected);
        self
    }

    fn assert_file_exists(&self, path: &str) -> &Self {
        let file_path = self.project_path.join(path);
        assert!(
            file_path.exists(),
            "Expected file '{}' to exist, but it doesn't",
            file_path.display()
        );
        self
    }

    fn assert_file_contains(&self, path: &str, content: &str) -> &Self {
        let file_path = self.project_path.join(path);
        assert!(
            file_path.exists(),
            "File '{}' doesn't exist",
            file_path.display()
        );

        let file_content = std::fs::read_to_string(&file_path)
            .unwrap_or_else(|e| panic!("Failed to read file '{}': {}", file_path.display(), e));

        assert!(
            file_content.contains(content),
            "Expected '{}' in file '{}', but content was:\n{}",
            content,
            file_path.display(),
            file_content
        );
        self
    }

    fn assert_file_snapshot_matches(&self, file_path: &str, snapshot_path: &str) -> &Self {
        let full_file_path = self.project_path.join(file_path);
        assert!(
            full_file_path.exists(),
            "File '{}' doesn't exist",
            full_file_path.display()
        );

        let file_content = std::fs::read_to_string(&full_file_path).unwrap_or_else(|e| {
            panic!("Failed to read file '{}': {}", full_file_path.display(), e)
        });

        let assertion = snapshot_assert_for_file(&full_file_path);

        let mut snapshot_full_path = std::env::current_dir().expect("Failed to get current dir");
        snapshot_full_path.push("tests");
        snapshot_full_path.push(snapshot_path);

        let expected = Data::read_from(&snapshot_full_path, None);
        let normalized =
            normalize_file_snapshot_content(&file_content, &full_file_path, &self.project_path);
        assertion.eq(normalized, expected);
        self
    }
}

impl TestOutputExt for TestFailure {
    fn assert_passed(&self, count: usize) -> &Self {
        let stdout = strip_ansi(&self.get_stdout());
        let passed_text = format!("✓ {count} passed");
        assert!(
            stdout.contains(&passed_text),
            "Expected '{passed_text}' in stdout, but got:\n{stdout}"
        );
        self
    }

    fn assert_failed(&self, count: usize) -> &Self {
        let stdout = strip_ansi(&self.get_stdout());
        let failed_text = format!("✗ {count} failed");
        assert!(
            stdout.contains(&failed_text),
            "Expected '{failed_text}' in stdout, but got:\n{stdout}"
        );
        self
    }

    fn assert_skipped(&self, count: usize) -> &Self {
        let stdout = strip_ansi(&self.get_stdout());
        let skipped_text = format!("○ {count} skipped");
        assert!(
            stdout.contains(&skipped_text),
            "Expected '{skipped_text}' in stdout, but got:\n{stdout}"
        );
        self
    }

    fn assert_todo(&self, count: usize) -> &Self {
        let stdout = strip_ansi(&self.get_stdout());
        let skipped_text = format!("□ {count} todo");
        assert!(
            stdout.contains(&skipped_text),
            "Expected '{skipped_text}' in stdout, but got:\n{stdout}"
        );
        self
    }

    fn assert_test_passed(&self, name: &str) -> &Self {
        let stdout = strip_ansi(&self.get_stdout());
        let test_line = format!("✓ {name}");
        assert!(
            stdout.contains(&test_line),
            "Expected test '{name}' to pass, but got:\n{stdout}"
        );
        self
    }

    fn assert_test_failed(&self, name: &str) -> &Self {
        let stdout = strip_ansi(&self.get_stdout());
        let test_line = format!("✗ {name}");
        assert!(
            stdout.contains(&test_line),
            "Expected test '{name}' to fail, but got:\n{stdout}"
        );
        self
    }

    fn assert_contains(&self, text: &str) -> &Self {
        let stdout = strip_ansi(&self.get_stdout());
        let stderr = strip_ansi(&self.get_stderr());
        assert!(
            stdout.contains(text) || stderr.contains(text),
            "Expected '{text}' in stdout/stderr, but got:\n{stdout}\n{stderr}",
        );
        self
    }

    fn assert_not_contains(&self, text: &str) -> &Self {
        let stdout = strip_ansi(&self.get_stdout());
        assert!(
            !stdout.contains(text),
            "Did not expect '{text}' in stdout, but got:\n{stdout}"
        );
        self
    }

    fn assert_stderr_contains(&self, text: &str) -> &Self {
        let stderr = strip_ansi(&self.get_stderr());
        assert!(
            stderr.contains(text),
            "Expected '{text}' in stderr, but got:\n{stderr}"
        );
        self
    }

    fn get_stdout(&self) -> String {
        String::from_utf8(self.output.get_output().stdout.clone())
            .expect("Failed to convert stdout to string")
    }

    fn get_stderr(&self) -> String {
        String::from_utf8(self.output.get_output().stderr.clone())
            .expect("Failed to convert stderr to string")
    }

    fn get_normalized_stdout(&self) -> String {
        normalize_output(&self.get_stdout(), &self.project_path)
    }

    fn get_normalized_stdout_keep_ansi(&self) -> String {
        normalize_output_keep_ansi(&self.get_stdout(), &self.project_path)
    }

    fn get_normalized_stderr(&self) -> String {
        normalize_output(&self.get_stderr(), &self.project_path)
    }

    fn assert_snapshot_matches(&self, path: &str) -> &Self {
        let normalized = self.get_normalized_stdout();
        let assertion = assertion();

        let mut snapshot_path = std::env::current_dir().expect("Failed to get current dir");
        snapshot_path.push("tests");
        snapshot_path.push(path);

        let expected = Data::read_from(&snapshot_path, None);
        assertion.eq(normalized, expected);
        self
    }

    fn assert_stdout_svg_snapshot_matches(&self, path: &str) -> &Self {
        let normalized = self.get_normalized_stdout_keep_ansi();
        let assertion = assertion();

        let mut snapshot_path = std::env::current_dir().expect("Failed to get current dir");
        snapshot_path.push("tests");
        snapshot_path.push(path);

        let expected = Data::read_from(&snapshot_path, Some(snapbox::data::DataFormat::TermSvg));

        assertion.eq(
            Data::from(normalized).coerce_to(snapbox::data::DataFormat::TermSvg),
            expected,
        );
        self
    }

    fn assert_stderr_svg_snapshot_matches(&self, path: &str) -> &Self {
        let normalized = normalize_output_keep_ansi(&self.get_stderr(), &self.project_path);
        let assertion = assertion();

        let mut snapshot_path = std::env::current_dir().expect("Failed to get current dir");
        snapshot_path.push("tests");
        snapshot_path.push(path);

        let expected = Data::read_from(&snapshot_path, Some(snapbox::data::DataFormat::TermSvg));

        assertion.eq(
            Data::from(normalized).coerce_to(snapbox::data::DataFormat::TermSvg),
            expected,
        );
        self
    }

    fn assert_stderr_snapshot_matches(&self, path: &str) -> &Self {
        let normalized = self.get_normalized_stderr();
        let assertion = assertion();

        let mut snapshot_path = std::env::current_dir().expect("Failed to get current dir");
        snapshot_path.push("tests");
        snapshot_path.push(path);

        let expected = Data::read_from(&snapshot_path, None);
        assertion.eq(normalized, expected);
        self
    }

    fn assert_file_exists(&self, path: &str) -> &Self {
        let file_path = self.project_path.join(path);
        assert!(
            file_path.exists(),
            "Expected file '{}' to exist, but it doesn't",
            file_path.display()
        );
        self
    }

    fn assert_file_contains(&self, path: &str, content: &str) -> &Self {
        let file_path = self.project_path.join(path);
        assert!(
            file_path.exists(),
            "File '{}' doesn't exist",
            file_path.display()
        );

        let file_content = std::fs::read_to_string(&file_path)
            .unwrap_or_else(|e| panic!("Failed to read file '{}': {}", file_path.display(), e));

        assert!(
            file_content.contains(content),
            "Expected '{}' in file '{}', but content was:\n{}",
            content,
            file_path.display(),
            file_content
        );
        self
    }

    fn assert_file_snapshot_matches(&self, file_path: &str, snapshot_path: &str) -> &Self {
        let full_file_path = self.project_path.join(file_path);
        assert!(
            full_file_path.exists(),
            "File '{}' doesn't exist",
            full_file_path.display()
        );

        let file_content = std::fs::read_to_string(&full_file_path).unwrap_or_else(|e| {
            panic!("Failed to read file '{}': {}", full_file_path.display(), e)
        });

        let assertion = snapshot_assert_for_file(&full_file_path);

        let mut snapshot_full_path = std::env::current_dir().expect("Failed to get current dir");
        snapshot_full_path.push("tests");
        snapshot_full_path.push(snapshot_path);

        let expected = Data::read_from(&snapshot_full_path, None);
        let normalized =
            normalize_file_snapshot_content(&file_content, &full_file_path, &self.project_path);
        assertion.eq(normalized, expected);
        self
    }
}
