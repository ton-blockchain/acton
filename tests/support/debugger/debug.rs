use crate::common::{acton_exe, acton_path_env};
use crate::regex;
use crate::support::debugger::{DebugMethod, DebuggerClient, run_script_file};
use crate::support::project::Project;
use crate::support::snapshots::normalize_output;
use crate::support::tempdir::create_tmp_dir;
use anyhow::Context;
use dap::types::StackFrame;
use std::cmp::max;
use std::ffi::OsString;
use std::fmt::Write as _;
use std::fs;
use std::net::TcpListener;
use std::path::PathBuf;
use std::process::{Child, Command, Output, Stdio};
use std::sync::Arc;
use std::thread;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use tempfile::TempDir;
use tvm_ffi::stack::{Tuple, TupleItem};

pub(crate) struct ProjectRef {
    pub path: PathBuf,
}

pub(crate) struct DebugBuilder {
    name: String,
    temp_dir: TempDir,
    code: String,
    project_path: Option<PathBuf>,
    project: Option<Project>,
    executable_file: Option<String>,
    debug_port: Option<u16>,
    method: DebugMethod,
    stack: Option<Tuple>,
    expected_execution_error: Option<String>,
}

struct CliDebugConfig {
    args: Vec<OsString>,
    current_dir: PathBuf,
    isolated_home: PathBuf,
    log_dir: PathBuf,
}

enum DebugProcess {
    InProcess(JoinHandle<anyhow::Result<String>>),
    Cli(Child),
}

enum CliDebugCommand {
    Test {
        path: Option<String>,
        filter: Option<String>,
    },
    Script {
        path: String,
    },
}

pub(crate) struct CliDebugBuilder {
    temp_dir: TempDir,
    project: Project,
    project_path: PathBuf,
    debug_port: Option<u16>,
    expected_execution_error: Option<String>,
    command: CliDebugCommand,
}

impl CliDebugBuilder {
    pub(crate) fn test(project: Project) -> Self {
        let project_path = project.path().to_path_buf();
        Self {
            temp_dir: create_tmp_dir(),
            project,
            project_path,
            debug_port: None,
            expected_execution_error: None,
            command: CliDebugCommand::Test {
                path: None,
                filter: None,
            },
        }
    }

    #[allow(dead_code)]
    pub(crate) fn script(project: Project, path: &str) -> Self {
        let project_path = project.path().to_path_buf();
        Self {
            temp_dir: create_tmp_dir(),
            project,
            project_path,
            debug_port: None,
            expected_execution_error: None,
            command: CliDebugCommand::Script {
                path: path.to_string(),
            },
        }
    }

    pub(crate) fn project_path(mut self, path: PathBuf) -> Self {
        self.project_path = path;
        self
    }

    pub(crate) fn path(mut self, path: &str) -> Self {
        if let CliDebugCommand::Test {
            path: test_path, ..
        } = &mut self.command
        {
            *test_path = Some(path.to_string());
        }
        self
    }

    pub(crate) fn filter(mut self, filter: &str) -> Self {
        if let CliDebugCommand::Test {
            filter: test_filter,
            ..
        } = &mut self.command
        {
            *test_filter = Some(filter.to_string());
        }
        self
    }

    #[allow(dead_code)]
    pub(crate) fn debug_port(mut self, debug_port: u16) -> Self {
        self.debug_port = Some(debug_port);
        self
    }

    #[allow(dead_code)]
    pub(crate) fn expect_execution_error(mut self, expected_error: &str) -> Self {
        self.expected_execution_error = Some(expected_error.to_string());
        self
    }

    pub(crate) fn build(self) -> DebugSession {
        let debug_port = self.debug_port.unwrap_or_else(reserve_free_port);
        let args = match self.command {
            CliDebugCommand::Test { path, filter } => {
                let mut args = vec![OsString::from("test")];
                if let Some(path) = path {
                    args.push(path.into());
                }
                if let Some(filter) = filter {
                    args.push("--filter".into());
                    args.push(filter.into());
                }
                args
            }
            CliDebugCommand::Script { path } => vec![OsString::from("script"), path.into()],
        };

        let project_ref = Arc::new(ProjectRef {
            path: self.project_path.clone(),
        });
        let cli = CliDebugConfig {
            args,
            current_dir: self.project_path.clone(),
            isolated_home: self.project.isolated_home().to_path_buf(),
            log_dir: self.project_path.join(".acton-test-logs"),
        };

        DebugSession {
            project_ref,
            code_path: PathBuf::new(),
            debug_port,
            debug_listener: None,
            method: DebugMethod::main(),
            stack: Tuple::empty(),
            expected_execution_error: self.expected_execution_error,
            _project: Some(self.project),
            _temp_dir: self.temp_dir,
            client_handle: None,
            cli: Some(cli),
        }
    }
}

impl DebugBuilder {
    pub(crate) fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            temp_dir: create_tmp_dir(),
            code: String::new(),
            project_path: None,
            project: None,
            executable_file: None,
            debug_port: None,
            method: DebugMethod::main(),
            stack: None,
            expected_execution_error: None,
        }
    }

    #[allow(dead_code)]
    pub(crate) fn code(mut self, code: &str) -> Self {
        self.code = code.to_string();
        self
    }

    pub(crate) fn project_ref(mut self, project: Project) -> Self {
        self.project = Some(project);
        self
    }

    #[allow(dead_code)]
    pub(crate) fn project_path(mut self, path: PathBuf) -> Self {
        self.project_path = Some(path);
        self
    }

    pub(crate) fn executable_file(mut self, file_name: &str) -> Self {
        self.executable_file = Some(file_name.to_string());
        self
    }

    #[allow(dead_code)]
    pub(crate) fn method_id(mut self, method_id: i32) -> Self {
        self.method = DebugMethod::from_id(method_id);
        self
    }

    pub(crate) fn method_name(mut self, method_name: &str) -> Self {
        self.method = DebugMethod::from_name(method_name);
        self
    }

    #[allow(dead_code)]
    pub(crate) fn stack(mut self, stack: Tuple) -> Self {
        self.stack = Some(stack);
        self
    }

    #[allow(dead_code)]
    pub(crate) fn accept_int(mut self, value: i32) -> Self {
        let mut tuple = Tuple::empty();
        tuple.push(TupleItem::Int(value.into()));
        self.stack = Some(tuple);
        self
    }

    #[allow(dead_code)]
    pub(crate) fn expect_execution_error(mut self, expected_error: &str) -> Self {
        self.expected_execution_error = Some(expected_error.to_string());
        self
    }

    pub(crate) fn build(self) -> DebugSession {
        let (default_project_path, project) = if let Some(project) = self.project {
            (project.path().to_path_buf(), Some(project))
        } else {
            let path = self.temp_dir.path().join(&self.name);
            fs::create_dir_all(&path).expect("Failed to create project dir");

            let code_path = path.join("debug_script.tolk");
            fs::write(&code_path, &self.code).expect("Failed to write debug script");
            (path, None)
        };
        let project_path = self.project_path.unwrap_or(default_project_path);

        let code_path = if let Some(executable_file) = self.executable_file {
            project_path.join(executable_file)
        } else if project.is_some() {
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

        DebugSession {
            project_ref,
            code_path,
            debug_port,
            debug_listener,
            method: self.method,
            stack: self.stack.unwrap_or_else(Tuple::empty),
            expected_execution_error: self.expected_execution_error,
            _project: project,
            _temp_dir: self.temp_dir,
            client_handle: None,
            cli: None,
        }
    }
}

pub(crate) struct DebugSession {
    project_ref: Arc<ProjectRef>,
    code_path: PathBuf,
    debug_port: u16,
    debug_listener: Option<TcpListener>,
    method: DebugMethod,
    stack: Tuple,
    expected_execution_error: Option<String>,
    _project: Option<Project>,
    _temp_dir: TempDir,
    client_handle: Option<DebugProcess>,
    cli: Option<CliDebugConfig>,
}

impl DebugSession {
    pub(crate) fn start(mut self) -> DebugClient {
        if let Some(cli) = self.cli.take() {
            return self.start_cli(cli);
        }

        let code = self.code_path.to_string_lossy().to_string();
        let port = self.debug_port;
        let debug_listener = self.debug_listener.take();

        let stack = self.stack.clone();
        let method = self.method.clone();
        let project_root = self.project_ref.path.clone();
        let handle = thread::spawn(move || {
            run_script_file(&code, &project_root, port, debug_listener, method, stack)
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

        self.client_handle = Some(DebugProcess::InProcess(handle));

        DebugClient {
            client: Some(client),
            session: self,
            trace: ExecutionTrace::new(),
            terminated: false,
        }
    }

    fn start_cli(mut self, cli: CliDebugConfig) -> DebugClient {
        let port = self.debug_port;
        let address = format!("127.0.0.1:{port}");
        let child = spawn_cli_debug_process(&cli, port)
            .unwrap_or_else(|err| panic!("Failed to spawn acton debug command: {err}"));
        let (client, child) = connect_cli_debugger(&address, child, Duration::from_secs(30))
            .unwrap_or_else(|err| {
                panic!("Failed to connect to acton debug command at {address}: {err}")
            });

        self.client_handle = Some(DebugProcess::Cli(child));

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

        match handle {
            DebugProcess::InProcess(handle) => self.finish_in_process(handle),
            DebugProcess::Cli(child) => self.finish_cli_process(child),
        }
    }

    fn finish_in_process(
        &mut self,
        handle: JoinHandle<anyhow::Result<String>>,
    ) -> anyhow::Result<()> {
        let mut client = self.client.take();
        if !handle.is_finished()
            && let Some(client) = client.as_mut()
        {
            match client.terminate() {
                Ok(()) => {}
                Err(err) if DebugActionExecutor::is_terminated_error(&err) => {}
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

    fn finish_cli_process(&mut self, mut child: Child) -> anyhow::Result<()> {
        let mut client = self.client.take();
        if child.try_wait()?.is_none()
            && let Some(client) = client.as_mut()
        {
            match client.terminate() {
                Ok(()) => {}
                Err(err) if DebugActionExecutor::is_terminated_error(&err) => {}
                Err(err) => {
                    if child.try_wait()?.is_none() {
                        return Err(err).context("failed to terminate acton debug command");
                    }
                }
            }
        }

        let output = wait_for_cli_debug_process(child, Duration::from_secs(15))?;
        if output.status.success() {
            if let Some(expected_error) = &self.session.expected_execution_error {
                anyhow::bail!(
                    "expected acton debug command to fail with '{expected_error}', but it succeeded"
                );
            }
            return Ok(());
        }

        let formatted_output = format_cli_debug_output(&output);
        if let Some(expected_error) = &self.session.expected_execution_error {
            if formatted_output.contains(expected_error) {
                return Ok(());
            }
            anyhow::bail!(
                "acton debug command failed with an unexpected error, expected '{expected_error}'\n{formatted_output}"
            );
        }

        anyhow::bail!("acton debug command failed\n{formatted_output}")
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

fn reserve_free_port() -> u16 {
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("Failed to reserve a debug port");
    listener
        .local_addr()
        .expect("Failed to inspect reserved debug port")
        .port()
}

fn spawn_cli_debug_process(cli: &CliDebugConfig, debug_port: u16) -> anyhow::Result<Child> {
    let mut command = Command::new(acton_exe());
    command
        .args(&cli.args)
        .arg("--debug")
        .arg("--debug-port")
        .arg(debug_port.to_string())
        .current_dir(&cli.current_dir)
        .env("PATH", acton_path_env())
        .env("HOME", &cli.isolated_home)
        .env("USERPROFILE", &cli.isolated_home)
        .env("ACTON_LOG_DIR", &cli.log_dir)
        .env("NO_COLOR", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    command
        .spawn()
        .with_context(|| format!("failed to spawn {}", acton_exe().display()))
}

fn connect_cli_debugger(
    address: &str,
    mut child: Child,
    timeout: Duration,
) -> anyhow::Result<(DebuggerClient, Child)> {
    let deadline = Instant::now() + timeout;
    loop {
        if child.try_wait()?.is_some() {
            let output = child.wait_with_output()?;
            anyhow::bail!(
                "acton debug command exited before DAP connection\n{}",
                format_cli_debug_output(&output)
            );
        }

        match DebuggerClient::connect(address) {
            Ok(client) => return Ok((client, child)),
            Err(err) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let output = child.wait_with_output()?;
                    anyhow::bail!(
                        "timed out waiting for DAP connection after {timeout:?}: {err}\n{}",
                        format_cli_debug_output(&output)
                    );
                }
            }
        }

        thread::sleep(Duration::from_millis(100));
    }
}

fn wait_for_cli_debug_process(mut child: Child, timeout: Duration) -> anyhow::Result<Output> {
    let deadline = Instant::now() + timeout;
    loop {
        if child.try_wait()?.is_some() {
            return child
                .wait_with_output()
                .context("failed to collect acton debug command output");
        }

        if Instant::now() >= deadline {
            let _ = child.kill();
            let output = child
                .wait_with_output()
                .context("failed to collect timed-out acton debug command output")?;
            anyhow::bail!(
                "timed out waiting for acton debug command to exit after {timeout:?}\n{}",
                format_cli_debug_output(&output)
            );
        }

        thread::sleep(Duration::from_millis(50));
    }
}

fn format_cli_debug_output(output: &Output) -> String {
    format!(
        "status: {}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
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
            || DebugClient::is_closed_transport_error(err)
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
        if stack_contains_ignored_snapshot_frame(&positions) {
            return Ok(());
        }
        let frame_id = positions.first().map_or(thread_id, |frame| frame.id);
        let variables = match self.client.variables(frame_id) {
            Ok(variables) => variables,
            Err(err) if Self::is_terminated_error(&err) => {
                *self.terminated = true;
                return Ok(());
            }
            Err(err) => return Err(err),
        };
        let variables = match self.variables_with_fields(variables) {
            Ok(variables) => variables,
            Err(err) if Self::is_terminated_error(&err) => {
                *self.terminated = true;
                return Ok(());
            }
            Err(err) => return Err(err),
        };
        let registers = match self.c4_register_with_fields(frame_id) {
            Ok(registers) => registers,
            Err(err) if Self::is_terminated_error(&err) => {
                *self.terminated = true;
                return Ok(());
            }
            Err(err) => return Err(err),
        };

        self.trace.add_step(positions, variables, registers, action);
        Ok(())
    }

    fn variables_with_fields(
        &mut self,
        variables: Vec<dap::types::Variable>,
    ) -> anyhow::Result<Vec<TraceVariable>> {
        variables
            .into_iter()
            .map(|variable| {
                let fields = if variable.variables_reference > 0 {
                    self.client.variables(variable.variables_reference)?
                } else {
                    Vec::new()
                };

                Ok(TraceVariable { variable, fields })
            })
            .collect()
    }

    fn c4_register_with_fields(&mut self, frame_id: i64) -> anyhow::Result<Option<TraceVariable>> {
        let Some(registers_scope) = self
            .client
            .scopes(frame_id)?
            .into_iter()
            .find(|scope| scope.name == "Registers")
        else {
            return Ok(None);
        };

        let registers = self.client.variables(registers_scope.variables_reference)?;
        let Some(c4) = registers
            .into_iter()
            .find(|variable| variable.name.starts_with("c4"))
        else {
            return Ok(None);
        };

        Ok(self.variables_with_fields(vec![c4])?.pop())
    }

    pub(crate) fn step_in(&mut self) -> anyhow::Result<()> {
        self.run_step("step_in".to_string(), |client| client.step_in(1))
    }

    #[allow(dead_code)]
    pub(crate) fn step_in_times(&mut self, count: usize) -> anyhow::Result<()> {
        for _ in 0..count {
            self.step_in()?;
        }
        Ok(())
    }

    pub(crate) fn step_in_until_terminated(&mut self, max_steps: usize) -> anyhow::Result<()> {
        for _ in 0..max_steps {
            if *self.terminated {
                return Ok(());
            }
            self.step_in()?;
        }

        if *self.terminated {
            Ok(())
        } else {
            anyhow::bail!("debugger did not terminate after {max_steps} step_in steps")
        }
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

    pub(crate) fn step_over_until_terminated(&mut self, max_steps: usize) -> anyhow::Result<()> {
        for _ in 0..max_steps {
            if *self.terminated {
                return Ok(());
            }
            self.step_over()?;
        }

        if *self.terminated {
            Ok(())
        } else {
            anyhow::bail!("debugger did not terminate after {max_steps} step_over steps")
        }
    }

    #[allow(dead_code)]
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

    #[allow(dead_code)]
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
    }
}

pub(crate) struct DebugResult {
    trace: ExecutionTrace,
    project_path: PathBuf,
}

impl DebugResult {
    pub(crate) fn assert_trace_snapshot_matches(&self, path: &str) -> &Self {
        let serialized = self.trace.serialize();
        let normalized =
            normalize_debug_trace_output(normalize_output(&serialized, &self.project_path));
        let assertion = crate::common::assertion();

        let mut snapshot_path = std::env::current_dir().expect("Failed to get current dir");
        snapshot_path.push("tests");
        snapshot_path.push(path);

        let expected = snapbox::Data::read_from(&snapshot_path, None);
        assertion.eq(normalized, expected);
        self
    }

    #[allow(dead_code)]
    pub(crate) fn trace(&self) -> &ExecutionTrace {
        &self.trace
    }
}

fn stack_contains_ignored_snapshot_frame(positions: &[StackFrame]) -> bool {
    positions.iter().any(|frame| {
        let name = frame
            .name
            .strip_suffix(" (inlined)")
            .unwrap_or(frame.name.as_str());
        let base_name = name.split_once('<').map_or(name, |(base, _)| base);

        is_ignored_snapshot_frame_name(name)
            || matches!(
                base_name,
                "SearchParams.hasPredicates"
                    | "SearchParams.toScalarFFITuple"
                    | "impl.findTransaction"
                    | "testing.treasury"
                    | "println"
                    | "ffi.println"
            )
    })
}

fn is_ignored_snapshot_frame_name(name: &str) -> bool {
    name == "expect"
        || name.starts_with("expect<")
        || name.starts_with("Expectation")
        || name.starts_with("Assert.")
        || name.starts_with("ffi.assert")
        || name.starts_with("toHave")
        || name.contains(".toHave")
}

fn normalize_debug_trace_output(content: String) -> String {
    let content = regex!(r"hash: 0x[0-9a-fA-F]+(?:\.\.\.)?")
        .replace_all(&content, "hash: [HASH]")
        .into_owned();
    let content = regex!(r"hash = 0x[0-9a-fA-F]+(?:\.\.\.)?")
        .replace_all(&content, "hash = [HASH]")
        .into_owned();
    let content = regex!(r"createdAt(?:: [^=]+)? = \d+")
        .replace_all(&content, "createdAt: uint32 = [TIMESTAMP]")
        .into_owned();
    regex!(r"raw = slice\{[0-9a-fA-F_]+\}")
        .replace_all(&content, "raw = slice{[HEX]}")
        .into_owned()
}

#[derive(Debug, Clone)]
pub(crate) struct ExecutionTrace {
    pub steps: Vec<ExecutionStep>,
}

#[derive(Debug, Clone)]
pub(crate) struct ExecutionStep {
    pub step_number: usize,
    pub variables: Vec<TraceVariable>,
    pub registers: Vec<TraceVariable>,
    pub action: String,
    pub code_context: Vec<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct TraceVariable {
    pub variable: dap::types::Variable,
    pub fields: Vec<dap::types::Variable>,
}

impl ExecutionTrace {
    fn new() -> Self {
        Self { steps: Vec::new() }
    }

    fn add_step(
        &mut self,
        positions: Vec<StackFrame>,
        variables: Vec<TraceVariable>,
        c4_register: Option<TraceVariable>,
        action: String,
    ) {
        let step_number = self.steps.len() + 1;
        let code_context = self.get_code_context(&positions);
        let registers = c4_register.into_iter().collect();
        self.steps.push(ExecutionStep {
            step_number,
            variables,
            registers,
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
            let Ok(content) = fs::read_to_string(path) else {
                return vec![format!("{path}:{}:{}", pos.line, pos.column)];
            };
            let content = content.lines().collect::<Vec<_>>();

            if line_idx < content.len() {
                let start_line = line_idx.saturating_sub(2);
                let end_line = (line_idx + 2).min(content.len());
                let mut context = Vec::new();

                for (i, line) in content.iter().enumerate().take(end_line).skip(start_line) {
                    let line_num = i + 1;
                    let context_line = if line.is_empty() {
                        format!("{line_num:3}|")
                    } else {
                        format!("{line_num:3}| {line}")
                    };
                    context.push(context_line);
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
            let _ = writeln!(result, "Step {} ({}):", step.step_number, step.action);

            if !step.code_context.is_empty() {
                result.push_str("  Code:\n");
                for line in &step.code_context {
                    let _ = writeln!(result, "    {line}");
                }
            }

            if !step.variables.is_empty() {
                result.push_str("  Variables:\n");
                for var in sorted_global_variables(&step.variables) {
                    append_trace_variable(&mut result, var);
                }
            }

            if !step.registers.is_empty() {
                result.push_str("  Registers:\n");
                for register in &step.registers {
                    append_trace_variable(&mut result, register);
                }
            }

            result.push('\n');
        }

        result.trim_end().to_string()
    }
}

fn append_trace_variable(result: &mut String, var: &TraceVariable) {
    append_dap_variable(result, "    ", &var.variable);
    for field in &var.fields {
        append_dap_variable(result, "      ", field);
    }
}

fn append_dap_variable(result: &mut String, indent: &str, var: &dap::types::Variable) {
    let label = match var.type_field.as_deref().filter(|ty| !ty.is_empty()) {
        Some(type_name) => format!("{}: {type_name}", var.name),
        None => var.name.clone(),
    };

    if var.value.is_empty() {
        let _ = writeln!(result, "{indent}{label}");
    } else {
        let _ = writeln!(result, "{indent}{label} = {}", var.value);
    }
}

fn sorted_global_variables(variables: &[TraceVariable]) -> Vec<&TraceVariable> {
    let mut variables = variables.iter().collect::<Vec<_>>();
    let mut idx = 0;
    while idx < variables.len() {
        if !variables[idx].variable.name.starts_with("global ") {
            idx += 1;
            continue;
        }

        let end = variables[idx..]
            .iter()
            .position(|var| !var.variable.name.starts_with("global "))
            .map_or(variables.len(), |offset| idx + offset);
        variables[idx..end].sort_by(|left, right| left.variable.name.cmp(&right.variable.name));
        idx = end;
    }

    variables
}
