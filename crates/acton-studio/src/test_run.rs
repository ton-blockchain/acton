use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;
use xxhash_rust::xxh3::xxh3_64;

pub const STUDIO_TEST_RUN_FORMAT_VERSION: u32 = 1;
pub const STUDIO_TEST_RUNS_PATH: &str = "/api/v1/test-runs";

const TEST_HISTORY_RELATIVE_PATH: &str = ".studio/tests/runs";
const TEST_TRACES_RELATIVE_PATH: &str = ".studio/tests/traces";
const TEST_OUTPUT_RELATIVE_PATH: &str = ".studio/tests/output";
const STUDIO_DAEMON_RELATIVE_PATH: &str = ".studio/daemon.json";

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum TestRunSource {
    Manual,
    Studio,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum TestRunStatus {
    Queued,
    Running,
    Passed,
    Failed,
    Cancelled,
}

impl TestRunStatus {
    #[must_use]
    pub const fn is_finished(self) -> bool {
        matches!(self, Self::Passed | Self::Failed | Self::Cancelled)
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TestRunStats {
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub skipped: usize,
    pub todo: usize,
    pub duration_ms: u64,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StudioTestExecutionLogs {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stdout: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stderr: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vm_log: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct StudioTestDuration {
    pub secs: u64,
    pub nanos: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct StudioTestReport {
    pub name: String,
    pub suite_name: String,
    pub file_path: PathBuf,
    pub row: usize,
    pub column: usize,
    pub duration: StudioTestDuration,
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detailed_message: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failed_transactions: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failed_transaction_context: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub location: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub execution: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trace_path: Option<String>,
    #[serde(default, skip_serializing_if = "execution_logs_are_empty")]
    pub execution_logs: StudioTestExecutionLogs,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TestRunRecord {
    pub format_version: u32,
    pub id: String,
    pub project_root: PathBuf,
    pub source: TestRunSource,
    pub status: TestRunStatus,
    pub command: Vec<String>,
    pub started_at: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    #[serde(default)]
    pub stats: TestRunStats,
    #[serde(default)]
    pub reports: Vec<StudioTestReport>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trace_dir: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl TestRunRecord {
    #[must_use]
    pub fn new(
        id: String,
        project_root: PathBuf,
        source: TestRunSource,
        command: Vec<String>,
        trace_dir: Option<PathBuf>,
    ) -> Self {
        Self {
            format_version: STUDIO_TEST_RUN_FORMAT_VERSION,
            id,
            project_root,
            source,
            status: TestRunStatus::Running,
            command,
            started_at: Utc::now(),
            finished_at: None,
            exit_code: None,
            stats: TestRunStats::default(),
            reports: Vec::new(),
            trace_dir,
            error: None,
        }
    }

    #[must_use]
    pub fn summary(&self) -> TestRunSummary {
        TestRunSummary {
            format_version: self.format_version,
            id: self.id.clone(),
            source: self.source,
            status: self.status,
            command: self.command.clone(),
            started_at: self.started_at,
            finished_at: self.finished_at,
            exit_code: self.exit_code,
            stats: self.stats.clone(),
            error: self.error.clone(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TestRunSummary {
    pub format_version: u32,
    pub id: String,
    pub source: TestRunSource,
    pub status: TestRunStatus,
    pub command: Vec<String>,
    pub started_at: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    pub stats: TestRunStats,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartTestRunRequest {
    #[serde(default)]
    pub paths: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filter: Option<String>,
    #[serde(default)]
    pub include: Vec<String>,
    #[serde(default)]
    pub exclude: Vec<String>,
    #[serde(default)]
    pub fail_fast: bool,
    #[serde(default)]
    pub save_traces: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TestDescriptorSummary {
    pub name: String,
    pub row: usize,
    pub column: usize,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TestIdentity {
    pub name: String,
    pub suite_name: String,
    pub file_path: PathBuf,
    pub row: usize,
    pub column: usize,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(
    tag = "type",
    content = "data",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum TestRunEvent {
    RunStarted {
        run: TestRunRecord,
    },
    SuiteStarted {
        file_path: PathBuf,
        tests: Vec<TestDescriptorSummary>,
    },
    SuiteFinished {
        file_path: PathBuf,
        stats: TestRunStats,
    },
    TestStarted {
        test: TestIdentity,
    },
    TestFinished {
        report: StudioTestReport,
    },
    RunFinished {
        run: TestRunRecord,
    },
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TestRunEventEnvelope {
    pub format_version: u32,
    pub run_id: String,
    pub sequence: u64,
    pub event: TestRunEvent,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(
    tag = "type",
    content = "data",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum TestRunStreamEvent {
    RunChanged {
        run: TestRunSummary,
    },
    Output {
        run_id: String,
        stream: TestOutputStream,
        chunk: String,
    },
    ReporterEvent {
        event: Box<TestRunEventEnvelope>,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum TestOutputStream {
    Stdout,
    Stderr,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TestRunOutput {
    pub stdout: String,
    pub stderr: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StudioDaemonDescriptor {
    pub protocol_version: u32,
    pub url: String,
    pub pid: u32,
}

#[must_use]
pub fn new_test_run_id() -> String {
    Uuid::new_v4().to_string()
}

#[must_use]
pub fn is_valid_test_run_id(run_id: &str) -> bool {
    !run_id.is_empty()
        && run_id.len() <= 128
        && run_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

#[must_use]
pub fn test_history_dir(project_root: &Path) -> PathBuf {
    project_root.join(TEST_HISTORY_RELATIVE_PATH)
}

#[must_use]
pub fn test_trace_dir(project_root: &Path, run_id: &str) -> PathBuf {
    project_root.join(TEST_TRACES_RELATIVE_PATH).join(run_id)
}

#[must_use]
pub fn test_output_paths(project_root: &Path, run_id: &str) -> (PathBuf, PathBuf) {
    let output_dir = project_root.join(TEST_OUTPUT_RELATIVE_PATH);
    (
        output_dir.join(format!("{run_id}.stdout.log")),
        output_dir.join(format!("{run_id}.stderr.log")),
    )
}

#[must_use]
pub fn studio_daemon_descriptor_path(project_root: &Path) -> PathBuf {
    project_root.join(STUDIO_DAEMON_RELATIVE_PATH)
}

pub fn persist_studio_daemon_descriptor(
    project_root: &Path,
    descriptor: &StudioDaemonDescriptor,
) -> io::Result<()> {
    let path = studio_daemon_descriptor_path(project_root);
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("Studio daemon path has no parent directory"))?;
    fs::create_dir_all(parent)?;
    write_json_atomically(&path, descriptor)
}

pub fn load_studio_daemon_descriptor(
    project_root: &Path,
) -> io::Result<Option<StudioDaemonDescriptor>> {
    let path = studio_daemon_descriptor_path(project_root);
    match fs::read(path) {
        Ok(contents) => serde_json::from_slice(&contents)
            .map(Some)
            .map_err(io::Error::other),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

pub fn remove_studio_daemon_descriptor(project_root: &Path, expected_pid: u32) -> io::Result<()> {
    let path = studio_daemon_descriptor_path(project_root);
    let Some(descriptor) = load_studio_daemon_descriptor(project_root)? else {
        return Ok(());
    };
    if descriptor.pid != expected_pid {
        return Ok(());
    }
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

#[must_use]
pub fn test_contract_artifact_file_name(contract_name: &str) -> String {
    let stem = safe_file_stem(contract_name, "contract");
    if stem == contract_name {
        return format!("{stem}.json");
    }

    let suffix = xxh3_64(contract_name.as_bytes());
    format!("{stem}-{suffix:016x}.json")
}

pub fn persist_test_run(project_root: &Path, run: &TestRunRecord) -> io::Result<()> {
    if !is_valid_test_run_id(&run.id) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Test run ID contains unsupported characters",
        ));
    }
    let history_dir = test_history_dir(project_root);
    fs::create_dir_all(&history_dir)?;
    let path = history_dir.join(format!("{}.json", run.id));
    write_json_atomically(&path, run)
}

pub fn load_test_runs(project_root: &Path) -> io::Result<BTreeMap<String, TestRunRecord>> {
    let history_dir = test_history_dir(project_root);
    let mut runs = BTreeMap::new();
    let entries = match fs::read_dir(history_dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(runs),
        Err(error) => return Err(error),
    };

    for entry in entries {
        let entry = entry?;
        if entry.path().extension().and_then(|value| value.to_str()) != Some("json") {
            continue;
        }
        let Ok(contents) = fs::read(entry.path()) else {
            continue;
        };
        let Ok(run) = serde_json::from_slice::<TestRunRecord>(&contents) else {
            continue;
        };
        if run.format_version == STUDIO_TEST_RUN_FORMAT_VERSION && is_valid_test_run_id(&run.id) {
            runs.insert(run.id.clone(), run);
        }
    }
    Ok(runs)
}

const fn execution_logs_are_empty(logs: &StudioTestExecutionLogs) -> bool {
    logs.stdout.is_none() && logs.stderr.is_none() && logs.vm_log.is_none()
}

fn safe_file_stem(name: &str, fallback: &str) -> String {
    let mut stem = String::with_capacity(name.len());
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
            stem.push(ch);
        } else {
            stem.push('_');
        }
    }

    let stem = stem.trim_matches('_');
    if stem.is_empty() {
        fallback.to_owned()
    } else {
        stem.to_owned()
    }
}

fn write_json_atomically(path: &Path, value: &impl Serialize) -> io::Result<()> {
    let temp_path = path.with_extension(format!("{}.tmp", std::process::id()));
    let contents = serde_json::to_vec(value).map_err(io::Error::other)?;
    fs::write(&temp_path, contents)?;
    fs::rename(&temp_path, path).or_else(|_| {
        fs::write(path, fs::read(&temp_path)?)?;
        fs::remove_file(&temp_path)
    })
}
