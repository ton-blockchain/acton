use crate::debugging::{DebuggerClient, SourcePosition, run_script_file};
use crate::support::snapshots::normalize_output;
use dap::types::StackFrame;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread;
use std::thread::JoinHandle;
use std::time::Duration;
use tempfile::TempDir;
use tvmffi::stack::{Tuple, TupleItem};

pub struct ProjectRef {
    pub path: PathBuf,
}

pub struct DebugBuilder {
    name: String,
    temp_dir: TempDir,
    code: String,
    project_path: Option<PathBuf>,
    script_file: Option<String>,
    debug_port: Option<u16>,
    stack: Option<Tuple>,
}

impl DebugBuilder {
    pub fn new(name: &str) -> Self {
        let mut temp_dir = TempDir::new().expect("Failed to create temp dir");
        temp_dir.disable_cleanup(true);
        Self {
            name: name.to_string(),
            temp_dir,
            code: String::new(),
            project_path: None,
            script_file: None,
            debug_port: None,
            stack: None,
        }
    }

    pub fn code(mut self, code: &str) -> Self {
        self.code = code.to_string();
        self
    }

    pub fn project<P: AsRef<Path>>(mut self, path: P) -> Self {
        self.project_path = Some(path.as_ref().to_path_buf());
        self
    }

    pub fn script_file(mut self, file_name: &str) -> Self {
        self.script_file = Some(file_name.to_string());
        self
    }

    pub fn stack(mut self, stack: Tuple) -> Self {
        self.stack = Some(stack);
        self
    }

    pub fn accept_int(mut self, value: i32) -> Self {
        let mut tuple = Tuple::empty();
        tuple.push(TupleItem::Int(value.into()));
        self.stack = Some(tuple);
        self
    }

    pub fn build(self) -> DebugSession {
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

        let debug_port = self.debug_port.unwrap_or_else(find_available_port);

        let project_ref = Arc::new(ProjectRef { path: project_path });
        let stack = self.stack.unwrap_or_else(Tuple::empty);

        DebugSession {
            project_ref,
            code_path,
            debug_port,
            stack,
            _temp_dir: self.temp_dir,
            client_handle: None,
        }
    }
}

fn find_available_port() -> u16 {
    use std::net::TcpListener;

    for port in 42075..43000 {
        if TcpListener::bind(("127.0.0.1", port)).is_ok() {
            return port;
        }
    }
    panic!("No available debug ports found");
}

pub struct DebugSession {
    project_ref: Arc<ProjectRef>,
    code_path: PathBuf,
    debug_port: u16,
    stack: Tuple,
    _temp_dir: TempDir,
    client_handle: Option<JoinHandle<()>>,
}

impl DebugSession {
    pub fn start(mut self) -> DebugClient {
        let code = self.code_path.to_string_lossy().to_string();
        let port = self.debug_port;

        let source_content = fs::read_to_string(&code).expect("Failed to read code file");
        let source_lines: Vec<String> = source_content.lines().map(|s| s.to_string()).collect();

        let stack = self.stack.clone();
        let handle = thread::spawn(move || {
            let result = run_script_file(&code, &source_content, port, stack)
                .expect("Failed to run debug script");
            println!("Debug execution finished: {}", result);
        });

        let address = format!("127.0.0.1:{}", port);
        let client = DebuggerClient::connect_with_retry(&address, Duration::from_millis(2000))
            .expect("Failed to connect to debug server");

        self.client_handle = Some(handle);

        DebugClient {
            client: Some(client),
            session: self,
            trace: ExecutionTrace::new(source_lines),
        }
    }

    pub fn join(mut self) -> anyhow::Result<()> {
        if let Some(handle) = self.client_handle.take() {
            handle.join().map_err(|_| anyhow::anyhow!("Join error"))?;
        }
        Ok(())
    }
}

pub struct DebugClient {
    client: Option<DebuggerClient>,
    pub session: DebugSession,
    trace: ExecutionTrace,
}

impl DebugClient {
    pub fn execute<F>(&mut self, actions: F) -> anyhow::Result<DebugResult>
    where
        F: FnOnce(&mut DebugActionExecutor) -> anyhow::Result<()>,
    {
        let mut executor = DebugActionExecutor {
            client: self.client.as_mut().unwrap(),
            trace: &mut self.trace,
        };
        executor.record_state_with_action("before".to_owned())?;

        actions(&mut executor)?;

        Ok(DebugResult {
            trace: self.trace.clone(),
            project_path: self.session.project_ref.path.clone(),
        })
    }

    pub fn terminate(mut self) -> anyhow::Result<()> {
        if let Some(mut client) = self.client.take() {
            client.terminate()
        } else {
            Ok(())
        }
    }
}

pub struct DebugActionExecutor<'a> {
    client: &'a mut DebuggerClient,
    trace: &'a mut ExecutionTrace,
}

impl<'a> DebugActionExecutor<'a> {
    fn record_state_with_action(&mut self, action: String) -> anyhow::Result<()> {
        let thread_id = 1;
        let positions = self.client.stack_trace(thread_id)?;
        let variables = self.client.variables(thread_id)?;

        self.trace.add_step(positions, variables, action);
        Ok(())
    }

    pub fn step_in(&mut self) -> anyhow::Result<()> {
        self.client.step_in(1)?;
        self.record_state_with_action("step_in".to_string())
    }

    pub fn step_over(&mut self) -> anyhow::Result<()> {
        self.client.step_over(1)?;
        self.record_state_with_action("step_over".to_string())
    }

    pub fn step_out(&mut self) -> anyhow::Result<()> {
        self.client.step_out(1)?;
        self.record_state_with_action("step_out".to_string())
    }

    pub fn continue_execution(&mut self) -> anyhow::Result<()> {
        self.client.continue_execution(1)?;
        Ok(())
        // self.record_state_with_action("continue".to_string())
    }

    pub fn assert_position(&mut self, expected: &SourcePosition) -> anyhow::Result<()> {
        self.client.assert_position(1, expected)
    }
}

pub struct DebugResult {
    trace: ExecutionTrace,
    project_path: PathBuf,
}

impl DebugResult {
    pub fn assert_trace_snapshot_matches(&self, path: &str) -> &Self {
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

    pub fn trace(&self) -> &ExecutionTrace {
        &self.trace
    }
}

#[derive(Debug, Clone)]
pub struct ExecutionTrace {
    pub steps: Vec<ExecutionStep>,
    pub source_code: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ExecutionStep {
    pub step_number: usize,
    pub positions: Vec<StackFrame>,
    pub variables: Vec<dap::types::Variable>,
    pub action: String,
    pub code_context: Vec<String>,
}

impl ExecutionTrace {
    fn new(source_code: Vec<String>) -> Self {
        Self {
            steps: Vec::new(),
            source_code,
        }
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
            positions,
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

                for i in start_line..end_line {
                    let line_num = i + 1;
                    context.push(format!("{:3}| {}", line_num, content[i]));
                }

                if line_idx >= start_line && line_idx < end_line {
                    let line_relative_idx = line_idx - start_line;
                    let col = (pos.column - 1) as usize;
                    let end_col = if let Some(end_column) = pos.end_column
                        && pos.end_line == Some(pos.line)
                    {
                        end_column as usize
                    } else {
                        col + 1
                    };
                    let code_line = &content[line_idx];

                    let mut pointer_line = String::new();
                    pointer_line.push_str(&" ".repeat(5));

                    if col < code_line.len() {
                        pointer_line.push_str(&" ".repeat(col));

                        if pos.end_line == Some(pos.line)
                            && end_col > col
                            && end_col <= code_line.len()
                        {
                            let underline_len = end_col - col;
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

    pub fn serialize(&self) -> String {
        let mut result = String::new();

        for step in &self.steps {
            result.push_str(&format!("Step {} ({}):\n", step.step_number, step.action));

            result.push_str(&format!(
                "  Bytecode position: {}\n",
                step.positions
                    .first()
                    .cloned()
                    .unwrap_or_default()
                    .instruction_pointer_reference
                    .unwrap_or("<unknown-position>".to_owned())
            ));

            if !step.code_context.is_empty() {
                result.push_str("  Code:\n");
                for line in &step.code_context {
                    result.push_str(&format!("    {}\n", line));
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
