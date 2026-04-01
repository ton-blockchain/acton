use crate::debugging::{DebuggerClient, run_script_file};
use crate::support::project::Project;
use crate::support::snapshots::normalize_output;
use crate::support::tempdir::create_tmp_dir;
use anyhow::Context;
use dap::types::StackFrame;
use std::cmp::max;
use std::fs;
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;
use std::thread::JoinHandle;
use std::time::Duration;
use tempfile::TempDir;
use tvmffi::stack::{Tuple, TupleItem};

pub(crate) struct ProjectRef {
    pub path: PathBuf,
}

pub(crate) struct DebugBuilder {
    name: String,
    temp_dir: TempDir,
    code: String,
    project_path: Option<PathBuf>,
    project: Option<Project>,
    script_file: Option<String>,
    debug_port: Option<u16>,
    stack: Option<Tuple>,
    expected_execution_error: Option<String>,
}

impl DebugBuilder {
    pub(crate) fn new(name: &str) -> Self {
        let temp_dir = create_tmp_dir();
        Self {
            name: name.to_string(),
            temp_dir,
            code: String::new(),
            project_path: None,
            project: None,
            script_file: None,
            debug_port: None,
            stack: None,
            expected_execution_error: None,
        }
    }

    pub(crate) fn code(mut self, code: &str) -> Self {
        self.code = code.to_string();
        self
    }

    pub(crate) fn project_ref(mut self, project: Project) -> Self {
        self.project_path = Some(project.path().to_path_buf());
        self.project = Some(project);
        self
    }

    pub(crate) fn script_file(mut self, file_name: &str) -> Self {
        self.script_file = Some(file_name.to_string());
        self
    }

    pub(crate) fn stack(mut self, stack: Tuple) -> Self {
        self.stack = Some(stack);
        self
    }

    pub(crate) fn accept_int(mut self, value: i32) -> Self {
        let mut tuple = Tuple::empty();
        tuple.push(TupleItem::Int(value.into()));
        self.stack = Some(tuple);
        self
    }

    pub(crate) fn expect_execution_error(mut self, expected_error: &str) -> Self {
        self.expected_execution_error = Some(expected_error.to_string());
        self
    }

    pub(crate) fn build(self) -> DebugSession {
        let project_path = if let Some(path) = &self.project_path {
            path.clone()
        } else {
            let path = self.temp_dir.path().join(&self.name);
            fs::create_dir_all(&path).expect("Failed to create project dir");

            let code_path = path.join("debug_script.tolk");
            fs::write(&code_path, &self.code).expect("Failed to write debug script");
            path
        };

        let code_path = if let Some(script_file) = self.script_file {
            if let Some(project_path) = self.project_path {
                project_path.join(script_file)
            } else {
                project_path.join(script_file)
            }
        } else if self.project_path.is_some() {
            project_path.join("main.tolk")
        } else {
            project_path.join("debug_script.tolk")
        };

        let (debug_port, debug_listener) = if let Some(port) = self.debug_port {
            (port, None)
        } else {
            let listener =
                TcpListener::bind(("127.0.0.1", 0)).expect("Failed to reserve a debug port");
            let port = listener
                .local_addr()
                .expect("Failed to inspect reserved debug port")
                .port();
            (port, Some(listener))
        };

        let project_ref = Arc::new(ProjectRef { path: project_path });
        let stack = self.stack.unwrap_or_else(Tuple::empty);

        DebugSession {
            project_ref,
            code_path,
            debug_port,
            debug_listener,
            stack,
            expected_execution_error: self.expected_execution_error,
            _project: self.project,
            _temp_dir: self.temp_dir,
            client_handle: None,
        }
    }
}

pub(crate) struct DebugSession {
    project_ref: Arc<ProjectRef>,
    code_path: PathBuf,
    debug_port: u16,
    debug_listener: Option<TcpListener>,
    stack: Tuple,
    expected_execution_error: Option<String>,
    _project: Option<Project>,
    _temp_dir: TempDir,
    client_handle: Option<JoinHandle<anyhow::Result<String>>>,
}

impl DebugSession {
    pub(crate) fn start(mut self) -> DebugClient {
        let code = self.code_path.to_string_lossy().to_string();
        let port = self.debug_port;
        let debug_listener = self.debug_listener.take();

        let source_content = fs::read_to_string(&code).expect("Failed to read code file");

        let stack = self.stack.clone();
        let handle = thread::spawn(move || {
            run_script_file(&code, &source_content, port, debug_listener, stack)
        });

        let address = format!("127.0.0.1:{port}");
        let client = match DebuggerClient::connect_with_retry(&address, Duration::from_secs(5)) {
            Ok(client) => client,
            Err(connect_err) => {
                let worker_result = handle.join();
                match worker_result {
                    Ok(Err(worker_err)) => {
                        panic!(
                            "Failed to connect to debug server: {connect_err}\nWorker exited early with: {worker_err}"
                        );
                    }
                    Ok(Ok(_)) => {
                        panic!(
                            "Failed to connect to debug server: {connect_err}\nWorker finished before debugger handshake"
                        );
                    }
                    Err(payload) => std::panic::resume_unwind(payload),
                }
            }
        };

        self.client_handle = Some(handle);

        DebugClient {
            client: Some(client),
            session: self,
            trace: ExecutionTrace::new(),
            terminated: false,
        }
    }
}

pub(crate) struct DebugClient {
    client: Option<DebuggerClient>,
    pub session: DebugSession,
    trace: ExecutionTrace,
    terminated: bool,
}

impl DebugClient {
    pub(crate) fn execute<F>(&mut self, actions: F) -> anyhow::Result<DebugResult>
    where
        F: FnOnce(&mut DebugActionExecutor) -> anyhow::Result<()>,
    {
        let mut executor = DebugActionExecutor {
            client: self.client.as_mut().unwrap(),
            trace: &mut self.trace,
            terminated: &mut self.terminated,
        };
        executor.record_state_with_action("before".to_owned())?;

        actions(&mut executor)?;
        self.finish_execution()?;

        Ok(DebugResult {
            trace: self.trace.clone(),
            project_path: self.session.project_ref.path.clone(),
        })
    }

    #[allow(dead_code)]
    pub(crate) fn terminate(mut self) -> anyhow::Result<()> {
        self.finish_execution()
    }

    fn finish_execution(&mut self) -> anyhow::Result<()> {
        let Some(handle) = self.session.client_handle.take() else {
            return Ok(());
        };

        let mut client = self.client.take();
        if !handle.is_finished()
            && let Some(client) = client.as_mut()
        {
            match client.terminate() {
                Ok(()) => {}
                Err(err)
                    if DebugActionExecutor::is_terminated_error(&err)
                        || Self::is_closed_transport_error(&err) => {}
                Err(err) => {
                    if !handle.is_finished() {
                        return Err(err).context("failed to terminate debug session");
                    }
                }
            }
        }

        match handle.join() {
            Ok(Ok(_output)) => {
                if let Some(expected_error) = &self.session.expected_execution_error {
                    anyhow::bail!(
                        "expected debug execution to fail with '{expected_error}', but it succeeded"
                    );
                }
                Ok(())
            }
            Ok(Err(err)) => {
                if let Some(expected_error) = &self.session.expected_execution_error {
                    if err.to_string().contains(expected_error) {
                        return Ok(());
                    }
                    return Err(err).context(format!(
                        "debug execution failed with an unexpected error, expected '{expected_error}'"
                    ));
                }
                Err(err).context("debug execution failed")
            }
            Err(payload) => std::panic::resume_unwind(payload),
        }
    }

    fn is_closed_transport_error(err: &anyhow::Error) -> bool {
        err.to_string().contains("Timeout waiting for response")
            || err.downcast_ref::<std::io::Error>().is_some_and(|io_err| {
                matches!(
                    io_err.kind(),
                    std::io::ErrorKind::BrokenPipe
                        | std::io::ErrorKind::ConnectionReset
                        | std::io::ErrorKind::NotConnected
                        | std::io::ErrorKind::UnexpectedEof
                )
            })
    }
}

impl Drop for DebugClient {
    fn drop(&mut self) {
        if thread::panicking() {
            return;
        }

        let _ = self.finish_execution();
    }
}

pub(crate) struct DebugActionExecutor<'a> {
    client: &'a mut DebuggerClient,
    trace: &'a mut ExecutionTrace,
    terminated: &'a mut bool,
}

impl DebugActionExecutor<'_> {
    fn is_terminated_error(err: &anyhow::Error) -> bool {
        err.to_string()
            .contains("The debugger terminated, probably because you stepped too many times")
    }

    fn run_step<T>(
        &mut self,
        action: String,
        step: impl FnOnce(&mut DebuggerClient) -> anyhow::Result<T>,
    ) -> anyhow::Result<()> {
        if *self.terminated {
            return Ok(());
        }

        match step(self.client) {
            Ok(_) => self.record_state_with_action(action),
            Err(err) if Self::is_terminated_error(&err) => {
                *self.terminated = true;
                Ok(())
            }
            Err(err) => Err(err),
        }
    }

    fn record_state_with_action(&mut self, action: String) -> anyhow::Result<()> {
        let thread_id = 1;
        let positions = match self.client.stack_trace(thread_id) {
            Ok(positions) => positions,
            Err(err) if Self::is_terminated_error(&err) => {
                *self.terminated = true;
                return Ok(());
            }
            Err(err) => return Err(err),
        };
        let variables = match self.client.variables(thread_id) {
            Ok(variables) => variables,
            Err(err) if Self::is_terminated_error(&err) => {
                *self.terminated = true;
                return Ok(());
            }
            Err(err) => return Err(err),
        };

        self.trace.add_step(positions, variables, action);
        Ok(())
    }

    pub(crate) fn step_in(&mut self) -> anyhow::Result<()> {
        self.run_step("step_in".to_string(), |client| client.step_in(1))
    }

    pub(crate) fn step_in_times(&mut self, count: usize) -> anyhow::Result<()> {
        for _ in 0..count {
            self.step_in()?;
        }
        Ok(())
    }

    pub(crate) fn step_over(&mut self) -> anyhow::Result<()> {
        self.run_step("step_over".to_string(), |client| client.step_over(1))
    }

    pub(crate) fn step_over_times(&mut self, count: usize) -> anyhow::Result<()> {
        for _ in 0..count {
            self.step_over()?;
        }
        Ok(())
    }

    pub(crate) fn step_out(&mut self) -> anyhow::Result<()> {
        self.run_step("step_out".to_string(), |client| client.step_out(1))
    }

    #[allow(dead_code)]
    pub(crate) fn step_out_times(&mut self, count: usize) -> anyhow::Result<()> {
        for _ in 0..count {
            self.step_out()?;
        }
        Ok(())
    }

    pub(crate) fn continue_execution(&mut self) -> anyhow::Result<()> {
        if *self.terminated {
            return Ok(());
        }

        match self.client.continue_execution(1) {
            Ok(_) => {}
            Err(err) if Self::is_terminated_error(&err) => {
                *self.terminated = true;
            }
            Err(err) => return Err(err),
        }
        Ok(())
        // self.record_state_with_action("continue".to_string())
    }
}

pub(crate) struct DebugResult {
    trace: ExecutionTrace,
    project_path: PathBuf,
}

impl DebugResult {
    pub(crate) fn assert_trace_snapshot_matches(&self, path: &str) -> &Self {
        let serialized = self.trace.serialize();
        let normalized = normalize_output(&serialized, &self.project_path);
        let assertion = crate::common::assertion();

        let mut snapshot_path = std::env::current_dir().expect("Failed to get current dir");
        snapshot_path.push("tests");
        snapshot_path.push(path);

        let expected = snapbox::Data::read_from(&snapshot_path, None);
        assertion.eq(normalized, expected);
        self
    }

    pub(crate) fn trace(&self) -> &ExecutionTrace {
        &self.trace
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ExecutionTrace {
    pub steps: Vec<ExecutionStep>,
}

#[derive(Debug, Clone)]
pub(crate) struct ExecutionStep {
    pub step_number: usize,
    pub variables: Vec<dap::types::Variable>,
    pub action: String,
    pub code_context: Vec<String>,
}

impl ExecutionTrace {
    fn new() -> Self {
        Self { steps: Vec::new() }
    }

    fn add_step(
        &mut self,
        positions: Vec<StackFrame>,
        variables: Vec<dap::types::Variable>,
        action: String,
    ) {
        let step_number = self.steps.len() + 1;
        let code_context = self.get_code_context(&positions);
        self.steps.push(ExecutionStep {
            step_number,
            variables,
            action,
            code_context,
        });
    }

    fn get_code_context(&self, positions: &[StackFrame]) -> Vec<String> {
        if let Some(pos) = positions.first() {
            let Some(source) = &pos.source else {
                return vec![];
            };
            let Some(path) = &source.path else {
                return vec![];
            };

            let line_idx = (pos.line - 1) as usize;
            let content = fs::read_to_string(path.clone())
                .unwrap_or_else(|_| panic!("cannot read file {path}"));
            let content = content.lines().collect::<Vec<_>>();

            if line_idx < content.len() {
                let start_line = line_idx.saturating_sub(3);
                let end_line = (line_idx + 4).min(content.len());
                let mut context = Vec::new();

                for (i, line) in content.iter().enumerate().take(end_line).skip(start_line) {
                    let line_num = i + 1;
                    context.push(format!("{line_num:3}| {line}"));
                }

                if line_idx >= start_line && line_idx < end_line {
                    let line_relative_idx = line_idx - start_line;
                    let col = (pos.column - 1) as usize;
                    let end_col = if let Some(end_column) = pos.end_column
                        && pos.end_line == Some(pos.line)
                    {
                        (end_column - 1) as usize
                    } else {
                        col
                    };
                    let code_line = &content[line_idx];

                    let mut pointer_line = String::new();
                    pointer_line.push_str(&" ".repeat(5));

                    if col < code_line.len() {
                        pointer_line.push_str(&" ".repeat(col));

                        if pos.end_line == Some(pos.line)
                            && end_col >= col
                            && end_col <= code_line.len()
                        {
                            let underline_len = max(end_col - col, 1);
                            pointer_line.push_str(&"^".repeat(underline_len));
                        } else {
                            pointer_line.push('^');
                        }
                    }

                    context.insert(line_relative_idx + 1, pointer_line);
                }

                context
            } else {
                vec![]
            }
        } else {
            vec![]
        }
    }

    pub(crate) fn serialize(&self) -> String {
        let mut result = String::new();

        for step in &self.steps {
            result.push_str(&format!("Step {} ({}):\n", step.step_number, step.action));

            if !step.code_context.is_empty() {
                result.push_str("  Code:\n");
                for line in &step.code_context {
                    result.push_str(&format!("    {line}\n"));
                }
            }

            if !step.variables.is_empty() {
                result.push_str("  Variables:\n");
                for var in &step.variables {
                    result.push_str(&format!("    {} = {}\n", var.name, var.value));
                }
            }

            result.push('\n');
        }

        result.trim_end().to_string()
    }
}
