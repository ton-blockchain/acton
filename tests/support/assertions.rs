use crate::common::{assertion, strip_ansi};
use crate::support::snapshots::normalize_output;
use snapbox::Data;
use snapbox::cmd::OutputAssert;
use std::path::PathBuf;

pub struct TestOutput {
    pub output: OutputAssert,
    pub project_path: PathBuf,
}

impl TestOutput {
    pub fn success(self) -> TestSuccess {
        let output = self.output.success();
        TestSuccess {
            output,
            project_path: self.project_path,
        }
    }

    pub fn failure(self) -> TestFailure {
        let output = self.output.failure();
        TestFailure {
            output,
            project_path: self.project_path,
        }
    }

    /// Assert specific exit code
    pub fn code(self, expected_code: i32) -> TestSuccess {
        let output = self.output.code(expected_code);
        TestSuccess {
            output,
            project_path: self.project_path,
        }
    }
}

pub struct TestSuccess {
    output: OutputAssert,
    project_path: PathBuf,
}

pub struct TestFailure {
    output: OutputAssert,
    project_path: PathBuf,
}

pub trait TestOutputExt {
    fn assert_passed(&self, count: usize) -> &Self;
    fn assert_failed(&self, count: usize) -> &Self;
    fn assert_skipped(&self, count: usize) -> &Self;
    fn assert_todo(&self, count: usize) -> &Self;
    fn assert_test_passed(&self, name: &str) -> &Self;
    fn assert_test_failed(&self, name: &str) -> &Self;
    fn assert_contains(&self, text: &str) -> &Self;
    fn assert_not_contains(&self, text: &str) -> &Self;
    fn assert_stderr_contains(&self, text: &str) -> &Self;
    fn get_stdout(&self) -> String;
    fn get_stderr(&self) -> String;
    fn get_normalized_stdout(&self) -> String;
    fn get_normalized_stderr(&self) -> String;
    fn assert_snapshot_matches(&self, path: &str) -> &Self;
    fn assert_stderr_snapshot_matches(&self, path: &str) -> &Self;
}

impl TestOutputExt for TestSuccess {
    fn assert_passed(&self, count: usize) -> &Self {
        let stdout = strip_ansi(&self.get_stdout());
        let passed_text = format!("✓ {} passed", count);
        assert!(
            stdout.contains(&passed_text),
            "Expected '{}' in stdout, but got:\n{}",
            passed_text,
            stdout
        );
        self
    }

    fn assert_failed(&self, count: usize) -> &Self {
        let stdout = strip_ansi(&self.get_stdout());
        let failed_text = format!("✗ {} failed", count);
        assert!(
            stdout.contains(&failed_text),
            "Expected '{}' in stdout, but got:\n{}",
            failed_text,
            stdout
        );
        self
    }

    fn assert_skipped(&self, count: usize) -> &Self {
        let stdout = strip_ansi(&self.get_stdout());
        let skipped_text = format!("○ {} skipped", count);
        assert!(
            stdout.contains(&skipped_text),
            "Expected '{}' in stdout, but got:\n{}",
            skipped_text,
            stdout
        );
        self
    }

    fn assert_todo(&self, count: usize) -> &Self {
        let stdout = strip_ansi(&self.get_stdout());
        let skipped_text = format!("□ {} todo", count);
        assert!(
            stdout.contains(&skipped_text),
            "Expected '{}' in stdout, but got:\n{}",
            skipped_text,
            stdout
        );
        self
    }

    fn assert_test_passed(&self, name: &str) -> &Self {
        let stdout = strip_ansi(&self.get_stdout());
        let test_line = format!("✓ {}", name);
        assert!(
            stdout.contains(&test_line),
            "Expected test '{}' to pass, but got:\n{}",
            name,
            stdout
        );
        self
    }

    fn assert_test_failed(&self, name: &str) -> &Self {
        let stdout = strip_ansi(&self.get_stdout());
        let test_line = format!("✗ {}", name);
        assert!(
            stdout.contains(&test_line),
            "Expected test '{}' to fail, but got:\n{}",
            name,
            stdout
        );
        self
    }

    fn assert_contains(&self, text: &str) -> &Self {
        let stdout = strip_ansi(&self.get_stdout());
        assert!(
            stdout.contains(text),
            "Expected '{}' in stdout, but got:\n{}",
            text,
            stdout
        );
        self
    }

    fn assert_not_contains(&self, text: &str) -> &Self {
        let stdout = strip_ansi(&self.get_stdout());
        assert!(
            !stdout.contains(text),
            "Did not expect '{}' in stdout, but got:\n{}",
            text,
            stdout
        );
        self
    }

    fn assert_stderr_contains(&self, text: &str) -> &Self {
        let stderr = strip_ansi(&self.get_stderr());
        assert!(
            stderr.contains(text),
            "Expected '{}' in stderr, but got:\n{}",
            text,
            stderr
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
}

impl TestOutputExt for TestFailure {
    fn assert_passed(&self, count: usize) -> &Self {
        let stdout = strip_ansi(&self.get_stdout());
        let passed_text = format!("✓ {} passed", count);
        assert!(
            stdout.contains(&passed_text),
            "Expected '{}' in stdout, but got:\n{}",
            passed_text,
            stdout
        );
        self
    }

    fn assert_failed(&self, count: usize) -> &Self {
        let stdout = strip_ansi(&self.get_stdout());
        let failed_text = format!("✗ {} failed", count);
        assert!(
            stdout.contains(&failed_text),
            "Expected '{}' in stdout, but got:\n{}",
            failed_text,
            stdout
        );
        self
    }

    fn assert_skipped(&self, count: usize) -> &Self {
        let stdout = strip_ansi(&self.get_stdout());
        let skipped_text = format!("○ {} skipped", count);
        assert!(
            stdout.contains(&skipped_text),
            "Expected '{}' in stdout, but got:\n{}",
            skipped_text,
            stdout
        );
        self
    }

    fn assert_todo(&self, count: usize) -> &Self {
        let stdout = strip_ansi(&self.get_stdout());
        let skipped_text = format!("□ {} todo", count);
        assert!(
            stdout.contains(&skipped_text),
            "Expected '{}' in stdout, but got:\n{}",
            skipped_text,
            stdout
        );
        self
    }

    fn assert_test_passed(&self, name: &str) -> &Self {
        let stdout = strip_ansi(&self.get_stdout());
        let test_line = format!("✓ {}", name);
        assert!(
            stdout.contains(&test_line),
            "Expected test '{}' to pass, but got:\n{}",
            name,
            stdout
        );
        self
    }

    fn assert_test_failed(&self, name: &str) -> &Self {
        let stdout = strip_ansi(&self.get_stdout());
        let test_line = format!("✗ {}", name);
        assert!(
            stdout.contains(&test_line),
            "Expected test '{}' to fail, but got:\n{}",
            name,
            stdout
        );
        self
    }

    fn assert_contains(&self, text: &str) -> &Self {
        let stdout = strip_ansi(&self.get_stdout());
        let stderr = strip_ansi(&self.get_stderr());
        assert!(
            stdout.contains(text) || stderr.contains(text),
            "Expected '{}' in stdout/stderr, but got:\n{}\n{}",
            text,
            stdout,
            stderr,
        );
        self
    }

    fn assert_not_contains(&self, text: &str) -> &Self {
        let stdout = strip_ansi(&self.get_stdout());
        assert!(
            !stdout.contains(text),
            "Did not expect '{}' in stdout, but got:\n{}",
            text,
            stdout
        );
        self
    }

    fn assert_stderr_contains(&self, text: &str) -> &Self {
        let stderr = strip_ansi(&self.get_stderr());
        assert!(
            stderr.contains(text),
            "Expected '{}' in stderr, but got:\n{}",
            text,
            stderr
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
}
