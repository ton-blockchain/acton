use acton_config::config::{Network, project_root as configured_project_root};
use acton_config::test::{CoverageFormat, TestConfig};
use anyhow::{Context, anyhow};
use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event as TermEvent, KeyCode, KeyEvent,
    KeyModifiers,
};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use dap::events::Event as DapEvent;
use dap::types::{Source, SourceBreakpoint, StackFrame, Variable};
use dap_client::DapClient;
use ratatui::Frame;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap};
use std::collections::{BTreeSet, HashMap, VecDeque};
use std::io::{BufRead, BufReader, Stdout, stdout};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, Instant};

const THREAD_ID: i64 = 1;
const MAX_LOG_LINES: usize = 300;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(120);
const TICK_RATE: Duration = Duration::from_millis(80);

pub fn test_tui_cmd(path: Option<String>, config: &TestConfig) -> anyhow::Result<()> {
    let debug_port = config.debug_port;
    let address = format!("127.0.0.1:{debug_port}");
    let (mut child, output_rx) = spawn_debug_test_child(path, config, debug_port)?;

    let mut app = DebugTuiApp::new(address, output_rx);
    let terminal_result = run_terminal_app(&mut app);

    if let Some(client) = app.client.as_mut() {
        let _ = client.terminate();
    }
    if child.try_wait()?.is_none() {
        let _ = child.kill();
    }
    let _ = child.wait();

    terminal_result
}

fn spawn_debug_test_child(
    path: Option<String>,
    config: &TestConfig,
    debug_port: u16,
) -> anyhow::Result<(Child, Receiver<ProcessOutput>)> {
    let exe = std::env::current_exe().context("failed to resolve current acton executable")?;
    let project_root = configured_project_root().to_path_buf();
    let mut cmd = Command::new(exe);
    cmd.arg("--project-root")
        .arg(project_root)
        .arg("--color")
        .arg("never")
        .arg("test");

    if let Some(path) = path {
        cmd.arg(path);
    }
    append_child_test_args(&mut cmd, config, debug_port);
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

    let mut child = cmd.spawn().context("failed to start debug test process")?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| anyhow!("debug test process stdout is not piped"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| anyhow!("debug test process stderr is not piped"))?;

    let (tx, rx) = mpsc::channel();
    spawn_output_reader(stdout, OutputStream::Stdout, tx.clone());
    spawn_output_reader(stderr, OutputStream::Stderr, tx);

    Ok((child, rx))
}

fn append_child_test_args(cmd: &mut Command, config: &TestConfig, debug_port: u16) {
    cmd.arg("--debug")
        .arg("--debug-port")
        .arg(debug_port.to_string())
        .arg("--reporter")
        .arg("dot");

    if let Some(filter) = &config.filter {
        cmd.arg("--filter").arg(filter);
    }
    for pattern in &config.exclude_patterns {
        cmd.arg("--exclude").arg(pattern);
    }
    for pattern in &config.include_patterns {
        cmd.arg("--include").arg(pattern);
    }
    for _ in 0..config.verbosity {
        cmd.arg("--verbose");
    }
    if config.show_bodies {
        cmd.arg("--show-bodies");
    }
    if config.clear_cache {
        cmd.arg("--clear-cache=true");
    }
    if config.fail_fast {
        cmd.arg("--fail-fast=true");
    }
    if let Some(backtrace) = config.backtrace {
        cmd.arg("--backtrace").arg(backtrace.to_string());
    }
    if let Some(fork_net) = &config.fork_net {
        cmd.arg("--fork-net").arg(format_network_for_cli(fork_net));
    }
    if let Some(block_number) = config.fork_block_number {
        cmd.arg("--fork-block-number").arg(block_number.to_string());
    }
    if let Some(seed) = config.fuzz_seed {
        cmd.arg("--fuzz-seed").arg(seed.to_string());
    }
    if let Some(trace_dir) = &config.save_test_trace {
        cmd.arg("--save-test-trace").arg(trace_dir);
    }
    if config.coverage {
        cmd.arg("--coverage");
        if let Some(format) = config.coverage_format {
            cmd.arg("--coverage-format")
                .arg(format_coverage_format(format));
        }
        if let Some(file) = &config.coverage_file {
            cmd.arg("--coverage-file").arg(file);
        }
        if let Some(percent) = config.coverage_minimum_percent {
            cmd.arg("--coverage-minimum-percent")
                .arg(percent.to_string());
        }
        if config.coverage_include_wrappers {
            cmd.arg("--coverage-include-wrappers");
        }
        if config.coverage_include_tests {
            cmd.arg("--coverage-include-tests");
        }
    }
}

fn format_network_for_cli(network: &Network) -> String {
    match network {
        Network::Custom(name) => format!("custom:{name}"),
        _ => network.to_string(),
    }
}

const fn format_coverage_format(format: CoverageFormat) -> &'static str {
    match format {
        CoverageFormat::Lcov => "lcov",
        CoverageFormat::Text => "text",
    }
}

#[derive(Clone, Copy)]
enum OutputStream {
    Stdout,
    Stderr,
}

struct ProcessOutput {
    stream: OutputStream,
    line: String,
}

fn spawn_output_reader<R>(reader: R, stream: OutputStream, tx: mpsc::Sender<ProcessOutput>)
where
    R: std::io::Read + Send + 'static,
{
    thread::spawn(move || {
        let reader = BufReader::new(reader);
        for line in reader.lines() {
            let Ok(line) = line else { break };
            let _ = tx.send(ProcessOutput { stream, line });
        }
    });
}

fn run_terminal_app(app: &mut DebugTuiApp) -> anyhow::Result<()> {
    let mut terminal = TerminalGuard::enter()?;
    let result = app.run(terminal.terminal());
    TerminalGuard::restore()?;
    result
}

struct TerminalGuard {
    terminal: Terminal<CrosstermBackend<Stdout>>,
}

impl TerminalGuard {
    fn enter() -> anyhow::Result<Self> {
        enable_raw_mode().context("failed to enable raw terminal mode")?;
        let mut stdout = stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)
            .context("failed to enter alternate terminal screen")?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;
        terminal.clear()?;
        Ok(Self { terminal })
    }

    const fn terminal(&mut self) -> &mut Terminal<CrosstermBackend<Stdout>> {
        &mut self.terminal
    }

    fn restore() -> anyhow::Result<()> {
        disable_raw_mode().context("failed to disable raw terminal mode")?;
        execute!(stdout(), LeaveAlternateScreen, DisableMouseCapture)
            .context("failed to leave alternate terminal screen")?;
        Ok(())
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(stdout(), LeaveAlternateScreen, DisableMouseCapture);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FocusPane {
    Source,
    Stack,
    Variables,
    Output,
}

struct VariableRow {
    depth: usize,
    name: String,
    value: String,
    type_name: Option<String>,
    variables_reference: i64,
    path: String,
    expanded: bool,
    is_scope: bool,
}

struct DebugTuiApp {
    address: String,
    output_rx: Receiver<ProcessOutput>,
    client: Option<DapClient>,
    last_connect_attempt: Instant,
    started_at: Instant,
    stack_frames: Vec<StackFrame>,
    selected_frame: usize,
    variables: Vec<VariableRow>,
    selected_variable: usize,
    variables_scroll: u16,
    variables_horizontal_scroll: u16,
    variables_view_height: usize,
    variables_view_width: usize,
    expanded_variables: BTreeSet<String>,
    source_path: Option<String>,
    source_lines: Vec<String>,
    current_line: Option<i64>,
    selected_line: usize,
    source_scroll: u16,
    output_scroll: u16,
    breakpoints: HashMap<String, BTreeSet<i64>>,
    logs: VecDeque<Line<'static>>,
    status: String,
    stop_reason: String,
    focus: FocusPane,
    should_quit: bool,
    terminated: bool,
}

impl DebugTuiApp {
    fn new(address: String, output_rx: Receiver<ProcessOutput>) -> Self {
        Self {
            address,
            output_rx,
            client: None,
            last_connect_attempt: Instant::now() - Duration::from_secs(1),
            started_at: Instant::now(),
            stack_frames: Vec::new(),
            selected_frame: 0,
            variables: Vec::new(),
            selected_variable: 0,
            variables_scroll: 0,
            variables_horizontal_scroll: 0,
            variables_view_height: 1,
            variables_view_width: 1,
            expanded_variables: BTreeSet::new(),
            source_path: None,
            source_lines: Vec::new(),
            current_line: None,
            selected_line: 1,
            source_scroll: 0,
            output_scroll: 0,
            breakpoints: HashMap::new(),
            logs: VecDeque::new(),
            status: "Starting test runner".to_owned(),
            stop_reason: "waiting for debugger".to_owned(),
            focus: FocusPane::Source,
            should_quit: false,
            terminated: false,
        }
    }

    fn run(&mut self, terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> anyhow::Result<()> {
        while !self.should_quit {
            self.drain_output();
            self.ensure_connected();
            self.drain_debugger_events();
            terminal.draw(|frame| self.draw(frame))?;

            if event::poll(TICK_RATE)?
                && let TermEvent::Key(key) = event::read()?
            {
                self.handle_key(key);
            }
        }
        Ok(())
    }

    fn drain_output(&mut self) {
        while let Ok(output) = self.output_rx.try_recv() {
            let style = match output.stream {
                OutputStream::Stdout => Style::default().fg(Color::Gray),
                OutputStream::Stderr => Style::default().fg(Color::Red),
            };
            self.push_log(Line::from(Span::styled(output.line, style)));
        }
    }

    fn ensure_connected(&mut self) {
        if self.client.is_some() || self.terminated {
            return;
        }
        if self.started_at.elapsed() > CONNECT_TIMEOUT {
            self.status = format!("Timed out connecting to {}", self.address);
            return;
        }
        if self.last_connect_attempt.elapsed() < Duration::from_millis(350) {
            return;
        }
        self.last_connect_attempt = Instant::now();

        match connect_debugger(&self.address) {
            Ok(mut client) => {
                self.status = format!("Connected to {}", self.address);
                "entry".clone_into(&mut self.stop_reason);
                if let Err(err) = self.refresh_debug_state(&mut client) {
                    self.status = format!("Failed to read debugger state: {err}");
                }
                self.client = Some(client);
            }
            Err(err) => {
                self.status = format!("Waiting for debugger on {} ({err})", self.address);
            }
        }
    }

    fn drain_debugger_events(&mut self) {
        let Some(client) = self.client.as_ref() else {
            return;
        };

        let mut events = Vec::new();
        loop {
            match client.try_receive_event(Duration::from_millis(0)) {
                Ok(Some(event)) => events.push(event),
                Ok(None) => break,
                Err(err) => {
                    self.status = format!("Debugger event error: {err}");
                    break;
                }
            }
        }

        for event in events {
            match event {
                DapEvent::Stopped(body) => {
                    self.stop_reason = format!("{:?}", body.reason);
                    let result = self.with_client(DebugTuiApp::refresh_debug_state);
                    if let Err(err) = result {
                        self.status = format!("Failed to refresh after stop: {err}");
                    }
                }
                DapEvent::Terminated(_) | DapEvent::Exited(_) => {
                    self.terminated = true;
                    "Debug session terminated".clone_into(&mut self.status);
                }
                _ => {}
            }
        }
    }

    fn with_client<T>(
        &mut self,
        f: impl FnOnce(&mut Self, &mut DapClient) -> anyhow::Result<T>,
    ) -> anyhow::Result<T> {
        let mut client = self
            .client
            .take()
            .ok_or_else(|| anyhow!("debugger is not connected"))?;
        let result = f(self, &mut client);
        self.client = Some(client);
        result
    }

    fn refresh_debug_state(&mut self, client: &mut DapClient) -> anyhow::Result<()> {
        let frames = client.stack_trace(THREAD_ID)?.stack_frames;
        self.stack_frames = frames;
        if self.selected_frame >= self.stack_frames.len() {
            self.selected_frame = self.stack_frames.len().saturating_sub(1);
        }
        self.refresh_source_from_selected_frame();
        self.refresh_variables(client)?;
        Ok(())
    }

    fn refresh_source_from_selected_frame(&mut self) {
        let Some(frame) = self.stack_frames.get(self.selected_frame) else {
            return;
        };
        self.current_line = Some(frame.line);
        self.selected_line = frame.line.max(1) as usize;

        let Some(path) = frame.source.as_ref().and_then(|source| source.path.clone()) else {
            self.source_path = None;
            self.source_lines.clear();
            return;
        };

        if self.source_path.as_deref() != Some(path.as_str()) {
            self.source_path = Some(path.clone());
            self.source_lines = std::fs::read_to_string(&path).map_or_else(
                |err| vec![format!("failed to read {path}: {err}")],
                |source| source.lines().map(ToOwned::to_owned).collect(),
            );
        }
        self.center_source_on_selected_line();
    }

    fn refresh_variables(&mut self, client: &mut DapClient) -> anyhow::Result<()> {
        let selected_path = self
            .variables
            .get(self.selected_variable)
            .map(|row| row.path.clone());
        self.variables.clear();
        let Some(frame) = self.stack_frames.get(self.selected_frame) else {
            return Ok(());
        };
        for scope in client.scopes(frame.id)?.scopes {
            let scope_path = format!("scope:{}", scope.name);
            self.variables.push(VariableRow {
                depth: 0,
                name: scope.name.clone(),
                value: String::new(),
                type_name: None,
                variables_reference: 0,
                path: scope_path.clone(),
                expanded: true,
                is_scope: true,
            });
            let vars = client.variables(scope.variables_reference)?.variables;
            for var in vars {
                self.push_variable_row(client, var, 1, &scope_path)?;
            }
        }
        self.selected_variable = selected_path
            .and_then(|path| self.variables.iter().position(|row| row.path == path))
            .unwrap_or_else(|| {
                self.selected_variable
                    .min(self.variables.len().saturating_sub(1))
            });
        self.ensure_selected_variable_visible();
        Ok(())
    }

    fn push_variable_row(
        &mut self,
        client: &mut DapClient,
        variable: Variable,
        depth: usize,
        parent_path: &str,
    ) -> anyhow::Result<()> {
        let variables_reference = variable.variables_reference;
        let path = format!("{parent_path}/{}", variable.name);
        let expanded = variables_reference > 0 && self.expanded_variables.contains(&path);
        self.variables.push(VariableRow {
            depth,
            name: variable.name,
            value: variable.value,
            type_name: variable.type_field,
            variables_reference,
            path: path.clone(),
            expanded,
            is_scope: false,
        });

        if expanded {
            for child in client.variables(variables_reference)?.variables {
                self.push_variable_row(client, child, depth + 1, &path)?;
            }
        }
        Ok(())
    }

    const fn center_source_on_selected_line(&mut self) {
        let line = self.selected_line.saturating_sub(1);
        self.source_scroll = line.saturating_sub(8) as u16;
    }

    fn handle_key(&mut self, key: KeyEvent) {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            self.should_quit = true;
            return;
        }

        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
            KeyCode::Tab => self.next_focus(),
            KeyCode::BackTab => self.prev_focus(),
            KeyCode::Char('s') | KeyCode::F(11) => self.step_in(),
            KeyCode::Char('n') | KeyCode::F(10) => self.step_over(),
            KeyCode::Char('o') => self.step_out(),
            KeyCode::Char('c') | KeyCode::F(5) => self.continue_execution(),
            KeyCode::Char('b') => self.toggle_breakpoint(),
            KeyCode::Enter => self.toggle_selected_variable(),
            KeyCode::Char('r') => {
                let _ = self.with_client(DebugTuiApp::refresh_debug_state);
            }
            KeyCode::Down | KeyCode::Char('j') => self.move_down(),
            KeyCode::Up | KeyCode::Char('k') => self.move_up(),
            KeyCode::Right | KeyCode::Char('l') => self.move_right(),
            KeyCode::Left | KeyCode::Char('h') => self.move_left(),
            KeyCode::PageDown => self.page_down(),
            KeyCode::PageUp => self.page_up(),
            KeyCode::Home => self.home(),
            KeyCode::End => self.end(),
            _ => {}
        }
    }

    const fn next_focus(&mut self) {
        self.focus = match self.focus {
            FocusPane::Source => FocusPane::Stack,
            FocusPane::Stack => FocusPane::Variables,
            FocusPane::Variables => FocusPane::Output,
            FocusPane::Output => FocusPane::Source,
        };
    }

    const fn prev_focus(&mut self) {
        self.focus = match self.focus {
            FocusPane::Source => FocusPane::Output,
            FocusPane::Stack => FocusPane::Source,
            FocusPane::Variables => FocusPane::Stack,
            FocusPane::Output => FocusPane::Variables,
        };
    }

    fn move_down(&mut self) {
        match self.focus {
            FocusPane::Source => {
                self.selected_line = (self.selected_line + 1).min(self.source_lines.len().max(1));
                self.center_source_on_selected_line();
            }
            FocusPane::Stack => {
                if self.selected_frame + 1 < self.stack_frames.len() {
                    self.selected_frame += 1;
                    self.refresh_source_from_selected_frame();
                    let _ = self.with_client(DebugTuiApp::refresh_variables);
                }
            }
            FocusPane::Variables => {
                if self.selected_variable + 1 < self.variables.len() {
                    self.selected_variable += 1;
                    self.ensure_selected_variable_visible();
                }
            }
            FocusPane::Output => self.output_scroll = self.output_scroll.saturating_add(1),
        }
    }

    fn move_up(&mut self) {
        match self.focus {
            FocusPane::Source => {
                self.selected_line = self.selected_line.saturating_sub(1).max(1);
                self.center_source_on_selected_line();
            }
            FocusPane::Stack => {
                self.selected_frame = self.selected_frame.saturating_sub(1);
                self.refresh_source_from_selected_frame();
                let _ = self.with_client(DebugTuiApp::refresh_variables);
            }
            FocusPane::Variables => {
                self.selected_variable = self.selected_variable.saturating_sub(1);
                self.ensure_selected_variable_visible();
            }
            FocusPane::Output => self.output_scroll = self.output_scroll.saturating_sub(1),
        }
    }

    fn page_down(&mut self) {
        match self.focus {
            FocusPane::Source => {
                self.selected_line = (self.selected_line + 10).min(self.source_lines.len().max(1));
                self.center_source_on_selected_line();
            }
            FocusPane::Variables => {
                let delta = self.variables_view_height.max(1);
                self.selected_variable =
                    (self.selected_variable + delta).min(self.variables.len().saturating_sub(1));
                self.ensure_selected_variable_visible();
            }
            FocusPane::Output => self.output_scroll = self.output_scroll.saturating_add(10),
            FocusPane::Stack => self.move_down(),
        }
    }

    fn page_up(&mut self) {
        match self.focus {
            FocusPane::Source => {
                self.selected_line = self.selected_line.saturating_sub(10).max(1);
                self.center_source_on_selected_line();
            }
            FocusPane::Variables => {
                let delta = self.variables_view_height.max(1);
                self.selected_variable = self.selected_variable.saturating_sub(delta);
                self.ensure_selected_variable_visible();
            }
            FocusPane::Output => self.output_scroll = self.output_scroll.saturating_sub(10),
            FocusPane::Stack => self.move_up(),
        }
    }

    fn move_right(&mut self) {
        if self.focus == FocusPane::Variables {
            self.variables_horizontal_scroll = self.variables_horizontal_scroll.saturating_add(8);
        }
    }

    fn move_left(&mut self) {
        if self.focus == FocusPane::Variables {
            self.variables_horizontal_scroll = self.variables_horizontal_scroll.saturating_sub(8);
        }
    }

    const fn home(&mut self) {
        match self.focus {
            FocusPane::Variables => self.variables_horizontal_scroll = 0,
            FocusPane::Output => self.output_scroll = 0,
            _ => {}
        }
    }

    fn end(&mut self) {
        match self.focus {
            FocusPane::Variables => {
                self.variables_horizontal_scroll =
                    self.max_variable_line_width()
                        .saturating_sub(self.variables_view_width)
                        .min(u16::MAX as usize) as u16;
            }
            FocusPane::Output => self.output_scroll = self.logs.len().min(u16::MAX as usize) as u16,
            _ => {}
        }
    }

    fn step_in(&mut self) {
        self.run_debugger_command("step in", |client| client.step_in(THREAD_ID));
    }

    fn step_over(&mut self) {
        self.run_debugger_command("step over", |client| client.step_over(THREAD_ID));
    }

    fn step_out(&mut self) {
        self.run_debugger_command("step out", |client| client.step_out(THREAD_ID));
    }

    fn continue_execution(&mut self) {
        self.run_debugger_command("continue", |client| {
            client.continue_execution(THREAD_ID).map(|_| ())
        });
    }

    fn run_debugger_command(
        &mut self,
        label: &str,
        command: impl FnOnce(&mut DapClient) -> anyhow::Result<()>,
    ) {
        let result = self.with_client(|app, client| {
            app.status = format!("Running {label}");
            command(client)?;
            app.status = format!("Waiting for stop after {label}");
            Ok(())
        });
        if let Err(err) = result {
            self.status = format!("{label} failed: {err}");
        }
    }

    fn toggle_selected_variable(&mut self) {
        if self.focus != FocusPane::Variables {
            return;
        }
        let Some(row) = self.variables.get(self.selected_variable) else {
            return;
        };
        if row.is_scope || row.variables_reference == 0 {
            return;
        }

        let path = row.path.clone();
        let name = row.name.clone();
        if self.expanded_variables.remove(&path) {
            self.status = format!("Collapsed {name}");
        } else {
            self.expanded_variables.insert(path);
            self.status = format!("Expanded {name}");
        }

        let result = self.with_client(DebugTuiApp::refresh_variables);
        if let Err(err) = result {
            self.status = format!("Failed to refresh variables: {err}");
        }
    }

    fn ensure_selected_variable_visible(&mut self) {
        let height = self.variables_view_height.max(1);
        let selected = self.selected_variable;
        let top = self.variables_scroll as usize;

        if selected < top {
            self.variables_scroll = selected.min(u16::MAX as usize) as u16;
        } else if selected >= top + height {
            self.variables_scroll = selected
                .saturating_add(1)
                .saturating_sub(height)
                .min(u16::MAX as usize) as u16;
        }
    }

    fn max_variable_line_width(&self) -> usize {
        self.variables
            .iter()
            .map(variable_line_width)
            .max()
            .unwrap_or(0)
    }

    fn toggle_breakpoint(&mut self) {
        let Some(path) = self.source_path.clone() else {
            "No source file selected".clone_into(&mut self.status);
            return;
        };
        let line = self.selected_line as i64;
        let lines = self.breakpoints.entry(path.clone()).or_default();
        if lines.contains(&line) {
            lines.remove(&line);
            self.status = format!("Removed breakpoint at {line}");
        } else {
            lines.insert(line);
            self.status = format!("Added breakpoint at {line}");
        }
        let breakpoints = lines
            .iter()
            .map(|line| SourceBreakpoint {
                line: *line,
                column: None,
                condition: None,
                hit_condition: None,
                log_message: None,
            })
            .collect::<Vec<_>>();
        let result = self.with_client(|_, client| {
            client.set_breakpoints(source_for_path(&path), breakpoints)?;
            Ok(())
        });
        if let Err(err) = result {
            self.status = format!("Failed to update breakpoints: {err}");
        }
    }

    fn push_log(&mut self, line: Line<'static>) {
        self.logs.push_back(line);
        while self.logs.len() > MAX_LOG_LINES {
            self.logs.pop_front();
        }
    }

    fn draw(&mut self, frame: &mut Frame<'_>) {
        let area = frame.area();
        let root = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(12),
                Constraint::Length(8),
                Constraint::Length(2),
            ])
            .split(area);

        self.draw_header(frame, root[0]);
        let body = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(66), Constraint::Percentage(34)])
            .split(root[1]);
        self.draw_source(frame, body[0]);

        let side = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(38), Constraint::Percentage(62)])
            .split(body[1]);
        self.draw_stack(frame, side[0]);
        self.draw_variables(frame, side[1]);
        self.draw_output(frame, root[2]);
        self.draw_footer(frame, root[3]);

        if self.client.is_none() && !self.terminated {
            self.draw_connecting_overlay(frame, area);
        }
    }

    fn draw_header(&self, frame: &mut Frame<'_>, area: Rect) {
        let title = Line::from(vec![
            Span::styled(
                " Acton Debug TUI ",
                Style::default().fg(Color::Black).bg(Color::Cyan),
            ),
            Span::raw(" "),
            Span::styled(&self.stop_reason, Style::default().fg(Color::Yellow)),
            Span::raw("  "),
            Span::styled(&self.status, Style::default().fg(Color::Gray)),
        ]);
        frame.render_widget(
            Paragraph::new(title)
                .block(Block::default().borders(Borders::ALL))
                .alignment(Alignment::Left),
            area,
        );
    }

    fn draw_source(&self, frame: &mut Frame<'_>, area: Rect) {
        let path = self
            .source_path
            .as_deref()
            .unwrap_or("waiting for source location");
        let title = format!(" Source: {} ", shorten_path(path));
        let lines = self
            .source_lines
            .iter()
            .enumerate()
            .map(|(idx, text)| {
                let line_no = idx + 1;
                let is_current = self.current_line == Some(line_no as i64);
                let is_selected = line_no == self.selected_line;
                let has_breakpoint = self
                    .source_path
                    .as_ref()
                    .and_then(|path| self.breakpoints.get(path))
                    .is_some_and(|lines| lines.contains(&(line_no as i64)));
                source_line(line_no, text, is_current, is_selected, has_breakpoint)
            })
            .collect::<Vec<_>>();
        let block = focused_block(title, self.focus == FocusPane::Source);
        frame.render_widget(
            Paragraph::new(lines)
                .block(block)
                .scroll((self.source_scroll, 0)),
            area,
        );
    }

    fn draw_stack(&self, frame: &mut Frame<'_>, area: Rect) {
        let items = self
            .stack_frames
            .iter()
            .enumerate()
            .map(|(idx, frame)| {
                let source = frame
                    .source
                    .as_ref()
                    .and_then(|source| source.path.as_deref())
                    .map_or_else(|| "<unknown>".to_owned(), shorten_path);
                let marker = if idx == self.selected_frame { ">" } else { " " };
                let style = if idx == self.selected_frame {
                    Style::default().fg(Color::Black).bg(Color::LightBlue)
                } else {
                    Style::default().fg(Color::Gray)
                };
                ListItem::new(Line::from(vec![
                    Span::styled(marker, style),
                    Span::raw(" "),
                    Span::styled(&frame.name, Style::default().fg(Color::White)),
                    Span::styled(
                        format!("  {}:{}", source, frame.line),
                        Style::default().fg(Color::DarkGray),
                    ),
                ]))
            })
            .collect::<Vec<_>>();
        frame.render_widget(
            List::new(items).block(focused_block(
                " Stack ".to_owned(),
                self.focus == FocusPane::Stack,
            )),
            area,
        );
    }

    fn draw_variables(&mut self, frame: &mut Frame<'_>, area: Rect) {
        self.variables_view_height = area.height.saturating_sub(2).max(1) as usize;
        self.variables_view_width = area.width.saturating_sub(2).max(1) as usize;
        self.ensure_selected_variable_visible();

        let lines = self
            .variables
            .iter()
            .enumerate()
            .map(|(idx, variable)| {
                let selected = idx == self.selected_variable && self.focus == FocusPane::Variables;
                let style = if selected {
                    Style::default()
                        .fg(Color::White)
                        .bg(Color::DarkGray)
                        .add_modifier(Modifier::BOLD)
                } else if variable.depth == 0 {
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                };
                let marker = if variable.is_scope {
                    "  "
                } else if variable.variables_reference > 0 && variable.expanded {
                    "▾ "
                } else if variable.variables_reference > 0 {
                    "▸ "
                } else {
                    "  "
                };
                let indent = "  ".repeat(variable.depth);
                let mut spans = vec![
                    Span::raw(indent),
                    Span::styled(marker, Style::default().fg(Color::Green)),
                    Span::styled(&variable.name, style),
                ];
                if let Some(type_name) = &variable.type_name {
                    spans.push(Span::styled(
                        format!(": {type_name}"),
                        Style::default().fg(Color::DarkGray),
                    ));
                }
                if !variable.value.is_empty() {
                    spans.push(Span::raw(" = "));
                    spans.push(Span::styled(
                        &variable.value,
                        Style::default().fg(Color::LightYellow),
                    ));
                }
                Line::from(spans).style(if selected {
                    Style::default().fg(Color::White).bg(Color::DarkGray)
                } else {
                    Style::default()
                })
            })
            .collect::<Vec<_>>();
        frame.render_widget(
            Paragraph::new(lines)
                .block(focused_block(
                    format!(
                        " Variables / Registers  x:{} ",
                        self.variables_horizontal_scroll
                    ),
                    self.focus == FocusPane::Variables,
                ))
                .scroll((self.variables_scroll, self.variables_horizontal_scroll)),
            area,
        );
    }

    fn draw_output(&self, frame: &mut Frame<'_>, area: Rect) {
        let lines = self.logs.iter().cloned().collect::<Vec<_>>();
        frame.render_widget(
            Paragraph::new(lines)
                .block(focused_block(
                    " Runner Output ".to_owned(),
                    self.focus == FocusPane::Output,
                ))
                .scroll((self.output_scroll, 0))
                .wrap(Wrap { trim: false }),
            area,
        );
    }

    fn draw_footer(&self, frame: &mut Frame<'_>, area: Rect) {
        let help = Line::from(vec![
            key("F10/n"),
            text(" step over  "),
            key("F11/s"),
            text(" step in  "),
            key("o"),
            text(" step out  "),
            key("F5/c"),
            text(" continue  "),
            key("b"),
            text(" breakpoint  "),
            key("Enter"),
            text(" expand  "),
            key("Tab"),
            text(" pane  "),
            key("j/k/h/l"),
            text(" move  "),
            key("q"),
            text(" quit"),
        ]);
        frame.render_widget(Paragraph::new(help), area);
    }

    fn draw_connecting_overlay(&self, frame: &mut Frame<'_>, area: Rect) {
        let popup = centered_rect(64, 20, area);
        frame.render_widget(Clear, popup);
        frame.render_widget(
            Paragraph::new(vec![
                Line::from(Span::styled(
                    "Waiting for test debugger",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
                Line::from(self.status.clone()),
                Line::from(""),
                Line::from(
                    "The child test process is building and will open the DAP port when ready.",
                ),
            ])
            .alignment(Alignment::Center)
            .block(Block::default().title(" Debug TUI ").borders(Borders::ALL)),
            popup,
        );
    }
}

fn connect_debugger(address: &str) -> anyhow::Result<DapClient> {
    let mut client = DapClient::connect(address)?;
    client.start()?;
    client.initialize()?;
    wait_for_initialized(&client)?;
    client.configuration_done()?;
    client.launch()?;
    wait_for_stopped(&client)?;
    Ok(client)
}

fn wait_for_initialized(client: &DapClient) -> anyhow::Result<()> {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if Instant::now() >= deadline {
            anyhow::bail!("timed out waiting for debugger initialization");
        }
        if let Some(event) = client.try_receive_event(Duration::from_millis(100))?
            && matches!(event, DapEvent::Initialized)
        {
            return Ok(());
        }
    }
}

fn wait_for_stopped(client: &DapClient) -> anyhow::Result<()> {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if Instant::now() >= deadline {
            anyhow::bail!("timed out waiting for debugger stop");
        }
        if let Some(event) = client.try_receive_event(Duration::from_millis(100))? {
            match event {
                DapEvent::Stopped(_) => return Ok(()),
                DapEvent::Terminated(_) | DapEvent::Exited(_) => {
                    anyhow::bail!("debugger terminated")
                }
                _ => {}
            }
        }
    }
}

fn source_for_path(path: &str) -> Source {
    Source {
        name: std::path::Path::new(path)
            .file_name()
            .map(|name| name.to_string_lossy().to_string()),
        path: Some(path.to_owned()),
        ..Default::default()
    }
}

fn source_line(
    line_no: usize,
    text: &str,
    is_current: bool,
    is_selected: bool,
    has_breakpoint: bool,
) -> Line<'static> {
    let gutter_style = if is_current {
        Style::default().fg(Color::Black).bg(Color::Yellow)
    } else if is_selected {
        Style::default().fg(Color::Black).bg(Color::DarkGray)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let marker = match (is_current, has_breakpoint) {
        (true, true) => "●▶",
        (true, false) => " ▶",
        (false, true) => "● ",
        (false, false) => "  ",
    };
    let mut spans = vec![
        Span::styled(format!("{line_no:>4} "), gutter_style),
        Span::styled(
            marker.to_owned(),
            breakpoint_style(has_breakpoint, is_current),
        ),
        Span::raw(" "),
    ];
    spans.extend(highlight_tolk(text));
    Line::from(spans)
}

fn breakpoint_style(has_breakpoint: bool, is_current: bool) -> Style {
    match (has_breakpoint, is_current) {
        (true, _) => Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        (false, true) => Style::default().fg(Color::Yellow),
        (false, false) => Style::default().fg(Color::DarkGray),
    }
}

fn variable_line_width(variable: &VariableRow) -> usize {
    let marker_width = 2;
    let type_width = variable
        .type_name
        .as_ref()
        .map_or(0, |type_name| type_name.chars().count() + 2);
    let value_width = if variable.value.is_empty() {
        0
    } else {
        variable.value.chars().count() + 3
    };

    variable.depth * 2 + marker_width + variable.name.chars().count() + type_width + value_width
}

fn highlight_tolk(text: &str) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let mut chars = text.char_indices().peekable();
    let mut plain_start = 0;

    while let Some((idx, ch)) = chars.next() {
        if ch == '/' && chars.peek().is_some_and(|(_, next)| *next == '/') {
            push_plain(&mut spans, text, plain_start, idx);
            spans.push(Span::styled(
                text[idx..].to_owned(),
                Style::default().fg(Color::DarkGray),
            ));
            return spans;
        }
        if ch == '"' {
            push_plain(&mut spans, text, plain_start, idx);
            let mut end = idx + ch.len_utf8();
            let mut escaped = false;
            for (sidx, sch) in chars.by_ref() {
                end = sidx + sch.len_utf8();
                if escaped {
                    escaped = false;
                } else if sch == '\\' {
                    escaped = true;
                } else if sch == '"' {
                    break;
                }
            }
            spans.push(Span::styled(
                text[idx..end].to_owned(),
                Style::default().fg(Color::Green),
            ));
            plain_start = end;
            continue;
        }
        if ch.is_ascii_digit() {
            push_plain(&mut spans, text, plain_start, idx);
            let mut end = idx + ch.len_utf8();
            while let Some((nidx, next)) = chars.peek().copied() {
                if next.is_ascii_hexdigit() || matches!(next, 'x' | 'X' | '_') {
                    chars.next();
                    end = nidx + next.len_utf8();
                } else {
                    break;
                }
            }
            spans.push(Span::styled(
                text[idx..end].to_owned(),
                Style::default().fg(Color::LightMagenta),
            ));
            plain_start = end;
            continue;
        }
        if is_ident_start(ch) {
            push_plain(&mut spans, text, plain_start, idx);
            let mut end = idx + ch.len_utf8();
            while let Some((nidx, next)) = chars.peek().copied() {
                if is_ident_continue(next) {
                    chars.next();
                    end = nidx + next.len_utf8();
                } else {
                    break;
                }
            }
            let word = &text[idx..end];
            let style = if is_tolk_keyword(word) {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else if is_tolk_builtin(word) {
                Style::default().fg(Color::LightBlue)
            } else {
                Style::default().fg(Color::White)
            };
            spans.push(Span::styled(word.to_owned(), style));
            plain_start = end;
        }
    }

    push_plain(&mut spans, text, plain_start, text.len());
    spans
}

fn push_plain(spans: &mut Vec<Span<'static>>, text: &str, start: usize, end: usize) {
    if start < end {
        spans.push(Span::styled(
            text[start..end].to_owned(),
            Style::default().fg(Color::White),
        ));
    }
}

const fn is_ident_start(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphabetic()
}

const fn is_ident_continue(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphanumeric()
}

fn is_tolk_keyword(word: &str) -> bool {
    matches!(
        word,
        "import"
            | "fun"
            | "get"
            | "const"
            | "global"
            | "struct"
            | "enum"
            | "type"
            | "return"
            | "if"
            | "else"
            | "repeat"
            | "while"
            | "do"
            | "throw"
            | "try"
            | "catch"
            | "match"
            | "var"
            | "val"
            | "mutate"
            | "assert"
            | "null"
            | "true"
            | "false"
    )
}

fn is_tolk_builtin(word: &str) -> bool {
    matches!(
        word,
        "int"
            | "bool"
            | "cell"
            | "slice"
            | "builder"
            | "tuple"
            | "address"
            | "coins"
            | "void"
            | "never"
            | "Cell"
            | "Slice"
            | "Builder"
            | "StateInit"
    )
}

fn focused_block(title: String, focused: bool) -> Block<'static> {
    let border_style = if focused {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(border_style)
}

fn key(value: &'static str) -> Span<'static> {
    Span::styled(value, Style::default().fg(Color::Black).bg(Color::Cyan))
}

fn text(value: &'static str) -> Span<'static> {
    Span::styled(value, Style::default().fg(Color::Gray))
}

fn shorten_path(path: &str) -> String {
    let project_root = configured_project_root().to_string_lossy().to_string();
    let path = path
        .strip_prefix(&project_root)
        .and_then(|path| path.strip_prefix(std::path::MAIN_SEPARATOR))
        .unwrap_or(path);
    if path.is_empty() {
        "Acton.toml".to_owned()
    } else {
        path.to_owned()
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, rect: Rect) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(rect);
    let horizontal = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vertical[1]);
    horizontal[1]
}
