use crate::common::acton_exe;
use crate::support::project::{ActonCommand, Project};
use reqwest::blocking::Client;
use serde_json::Value;
use std::fs::{self, File, OpenOptions};
use std::io::Read;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

const DEFAULT_READY_TIMEOUT: Duration = Duration::from_secs(15);
const STOP_TIMEOUT: Duration = Duration::from_secs(3);

pub(crate) struct LocalnetBuilder<'a> {
    project: &'a Project,
    current_dir: PathBuf,
    port: u16,
    port_reservation: Option<PortReservation>,
    args: Vec<String>,
    auth_token: Option<String>,
    ready_timeout: Duration,
}

#[allow(dead_code)]
impl Project {
    pub(crate) fn localnet(&self) -> LocalnetBuilder<'_> {
        LocalnetBuilder::new(self)
    }
}

#[allow(dead_code)]
impl<'a> LocalnetBuilder<'a> {
    fn new(project: &'a Project) -> Self {
        let port_reservation = reserve_available_port_pair();
        let port = port_reservation.port;
        Self {
            project,
            current_dir: project.path().to_path_buf(),
            port,
            port_reservation: Some(port_reservation),
            args: Vec::new(),
            auth_token: None,
            ready_timeout: DEFAULT_READY_TIMEOUT,
        }
    }

    pub(crate) fn current_dir(mut self, path: impl AsRef<Path>) -> Self {
        self.current_dir = path.as_ref().to_path_buf();
        self
    }

    pub(crate) fn port(mut self, port: u16) -> Self {
        self.port = port;
        self.port_reservation = None;
        self
    }

    pub(crate) fn arg(mut self, arg: impl Into<String>) -> Self {
        self.args.push(arg.into());
        self
    }

    pub(crate) fn args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.args.extend(args.into_iter().map(Into::into));
        self
    }

    pub(crate) fn ready_timeout(mut self, timeout: Duration) -> Self {
        self.ready_timeout = timeout;
        self
    }

    pub(crate) fn require_auth(mut self) -> Self {
        self.args.push("--require-auth".to_owned());
        self.auth_token = Some("test-localnet-auth-token".to_owned());
        self
    }

    pub(crate) fn before_start<F>(self, configure: F) -> Self
    where
        F: FnOnce(ActonCommand) -> ActonCommand,
    {
        configure(self.project.acton()).run().success();
        self
    }

    pub(crate) fn start(self) -> LocalnetHandle {
        let LocalnetBuilder {
            project,
            current_dir,
            port,
            port_reservation,
            args,
            auth_token,
            ready_timeout,
        } = self;

        let mut cmd = Command::new(acton_exe());
        cmd.arg("localnet")
            .arg("start")
            .arg("--port")
            .arg(port.to_string());
        if !args.iter().any(|arg| arg == "--block-interval-ms") {
            cmd.arg("--block-interval-ms").arg("50");
        }
        cmd.args(&args)
            .current_dir(&current_dir)
            .env("NO_COLOR", "1")
            .env("HOME", project.isolated_home())
            .env("USERPROFILE", project.isolated_home())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(auth_token) = auth_token.as_deref() {
            cmd.env("ACTON_LOCALNET_AUTH_TOKEN", auth_token);
        }

        let port_locks = release_port_reservation(port_reservation);
        let child = cmd.spawn().unwrap_or_else(|e| {
            panic!("Failed to start `acton localnet start --port {port}`: {e}")
        });

        let mut handle = LocalnetHandle {
            child: Some(child),
            port,
            base_url: format!("http://127.0.0.1:{port}"),
            auth_token,
            client: Client::builder()
                .timeout(Duration::from_secs(5))
                .build()
                .expect("Failed to create HTTP client for localnet tests"),
            _port_locks: port_locks,
        };

        match handle.wait_until_ready(ready_timeout) {
            Ok(base_url) => {
                handle.base_url = base_url;
            }
            Err(err) => {
                let logs = handle.terminate_and_collect_output();
                panic!("Localnet failed to become ready on port {port}: {err}\n{logs}");
            }
        }

        handle
    }
}

struct PortReservation {
    port: u16,
    _http_listener: TcpListener,
    _liteapi_listener: TcpListener,
    locks: PortLocks,
}

struct PortLocks {
    _locks: Vec<PortLock>,
}

struct PortLock {
    path: PathBuf,
    _file: File,
}

impl PortLocks {
    fn try_acquire(ports: &[u16]) -> Option<Self> {
        let dir = PathBuf::from("/tmp/acton-localnet-test-ports");
        fs::create_dir_all(&dir).expect("Failed to create localnet test port lock directory");

        let mut locks = Vec::with_capacity(ports.len());
        for port in ports {
            let path = dir.join(format!("port-{port}.lock"));
            let Ok(file) = OpenOptions::new().write(true).create_new(true).open(&path) else {
                return None;
            };
            locks.push(PortLock { path, _file: file });
        }

        Some(Self { _locks: locks })
    }
}

impl Drop for PortLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

pub(crate) struct LocalnetHandle {
    child: Option<Child>,
    port: u16,
    base_url: String,
    auth_token: Option<String>,
    client: Client,
    _port_locks: Option<PortLocks>,
}

#[allow(dead_code)]
impl LocalnetHandle {
    pub(crate) fn port(&self) -> u16 {
        self.port
    }

    pub(crate) fn base_url(&self) -> String {
        self.base_url.clone()
    }

    pub(crate) fn auth_token(&self) -> Option<&str> {
        self.auth_token.as_deref()
    }

    pub(crate) fn get_json(&self, path: &str) -> Value {
        let url = format!("{}{}", self.base_url(), normalize_path(path));
        let response = self
            .with_auth(self.client.get(&url))
            .send()
            .unwrap_or_else(|e| panic!("Failed GET {url}: {e}"));
        let status = response.status();
        let body = response
            .text()
            .unwrap_or_else(|e| panic!("Failed to read GET {url} response body: {e}"));
        assert!(
            status.is_success(),
            "GET {url} failed with status {status}: {body}"
        );
        serde_json::from_str(&body)
            .unwrap_or_else(|e| panic!("GET {url} returned invalid JSON: {e}\n{body}"))
    }

    pub(crate) fn get_json_with_status(&self, path: &str) -> (u16, Value) {
        let url = format!("{}{}", self.base_url(), normalize_path(path));
        let response = self
            .with_auth(self.client.get(&url))
            .send()
            .unwrap_or_else(|e| panic!("Failed GET {url}: {e}"));
        let status = response.status().as_u16();
        let body = response
            .text()
            .unwrap_or_else(|e| panic!("Failed to read GET {url} response body: {e}"));
        let json = serde_json::from_str(&body)
            .unwrap_or_else(|e| panic!("GET {url} returned invalid JSON: {e}\n{body}"));
        (status, json)
    }

    pub(crate) fn get_json_error(&self, path: &str) -> Value {
        let (status, json) = self.get_json_with_status(path);
        assert!(status >= 400, "GET {path} unexpectedly succeeded: {json}");
        json
    }

    pub(crate) fn post_json(&self, path: &str, payload: &Value) -> Value {
        let url = format!("{}{}", self.base_url(), normalize_path(path));
        let response = self
            .with_auth(self.client.post(&url).json(payload))
            .send()
            .unwrap_or_else(|e| panic!("Failed POST {url}: {e}"));
        let status = response.status();
        let body = response
            .text()
            .unwrap_or_else(|e| panic!("Failed to read POST {url} response body: {e}"));
        assert!(
            status.is_success(),
            "POST {url} failed with status {status}: {body}"
        );
        serde_json::from_str(&body)
            .unwrap_or_else(|e| panic!("POST {url} returned invalid JSON: {e}\n{body}"))
    }

    pub(crate) fn post_json_with_status(&self, path: &str, payload: &Value) -> (u16, Value) {
        let url = format!("{}{}", self.base_url(), normalize_path(path));
        let response = self
            .with_auth(self.client.post(&url).json(payload))
            .send()
            .unwrap_or_else(|e| panic!("Failed POST {url}: {e}"));
        let status = response.status().as_u16();
        let body = response
            .text()
            .unwrap_or_else(|e| panic!("Failed to read POST {url} response body: {e}"));
        let json = serde_json::from_str(&body)
            .unwrap_or_else(|e| panic!("POST {url} returned invalid JSON: {e}\n{body}"));
        (status, json)
    }

    pub(crate) fn post_json_error(&self, path: &str, payload: &Value) -> Value {
        let (status, json) = self.post_json_with_status(path, payload);
        assert!(status >= 400, "POST {path} unexpectedly succeeded: {json}");
        json
    }

    pub(crate) fn stop(mut self) {
        self.terminate();
    }

    pub(crate) fn wait_until_address_state_active(&self, address: &str, timeout: Duration) {
        let query = format!("/api/v2/getAddressState?address={address}");
        let deadline = Instant::now() + timeout;
        loop {
            let response = self.get_json(&query);
            if response["ok"].as_bool() == Some(true)
                && response["result"].as_str() == Some("active")
            {
                return;
            }
            assert!(
                Instant::now() < deadline,
                "Timed out waiting for address `{address}` to become active:\n{}",
                serde_json::to_string_pretty(&response).unwrap_or_default()
            );
            thread::sleep(Duration::from_millis(200));
        }
    }

    fn wait_until_ready(&mut self, timeout: Duration) -> Result<String, String> {
        let deadline = Instant::now() + timeout;
        let probe_url = format!("http://127.0.0.1:{}/api/v2/getMasterchainInfo", self.port);

        loop {
            if let Some(status) = self
                .child_mut()
                .try_wait()
                .map_err(|e| format!("Failed to poll Localnet process: {e}"))?
            {
                return Err(format!("Localnet exited before ready with status {status}"));
            }

            if let Ok(response) = self.with_auth(self.client.get(&probe_url)).send()
                && response.status().is_success()
                && let Ok(json) = response.json::<Value>()
                && json.get("ok").and_then(Value::as_bool) == Some(true)
            {
                let base_url = probe_url.trim_end_matches("/api/v2/getMasterchainInfo");
                return Ok(base_url.to_string());
            }

            if Instant::now() >= deadline {
                return Err(format!("Timed out waiting for readiness probe {probe_url}"));
            }

            thread::sleep(Duration::from_millis(100));
        }
    }

    fn with_auth(
        &self,
        request: reqwest::blocking::RequestBuilder,
    ) -> reqwest::blocking::RequestBuilder {
        match self.auth_token.as_deref() {
            Some(token) => request.bearer_auth(token),
            None => request,
        }
    }

    fn child_mut(&mut self) -> &mut Child {
        self.child
            .as_mut()
            .expect("Localnet child process is not available")
    }

    fn terminate(&mut self) {
        let Some(child) = self.child.as_mut() else {
            return;
        };

        match child.try_wait() {
            Ok(None) => {}
            Ok(Some(_)) | Err(_) => return,
        }

        send_interrupt(child);
        let deadline = Instant::now() + STOP_TIMEOUT;
        while matches!(child.try_wait(), Ok(None)) {
            if Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                break;
            }
            thread::sleep(Duration::from_millis(50));
        }
    }

    fn terminate_and_collect_output(&mut self) -> String {
        self.terminate();
        let (stdout, stderr) = take_child_output(self.child.as_mut());
        format!("Localnet stdout:\n{stdout}\n\nLocalnet stderr:\n{stderr}")
    }
}

impl Drop for LocalnetHandle {
    fn drop(&mut self) {
        self.terminate();
    }
}

fn reserve_available_port_pair() -> PortReservation {
    for _ in 0..100 {
        let http_listener = TcpListener::bind(("127.0.0.1", 0))
            .expect("Failed to reserve an ephemeral port for localnet tests");
        let port = http_listener
            .local_addr()
            .expect("Failed to read ephemeral port address")
            .port();
        let Some(liteapi_port) = port.checked_add(1) else {
            continue;
        };
        let Some(locks) = PortLocks::try_acquire(&[port, liteapi_port]) else {
            continue;
        };
        if let Ok(liteapi_listener) = TcpListener::bind(("127.0.0.1", liteapi_port)) {
            return PortReservation {
                port,
                _http_listener: http_listener,
                _liteapi_listener: liteapi_listener,
                locks,
            };
        }
    }

    panic!("Failed to reserve adjacent ephemeral ports for localnet tests");
}

fn release_port_reservation(port_reservation: Option<PortReservation>) -> Option<PortLocks> {
    let reservation = port_reservation?;
    let PortReservation {
        _http_listener,
        _liteapi_listener,
        locks,
        ..
    } = reservation;
    drop(_http_listener);
    drop(_liteapi_listener);
    Some(locks)
}

fn normalize_path(path: &str) -> String {
    if path.starts_with('/') {
        path.to_string()
    } else {
        format!("/{path}")
    }
}

#[cfg(unix)]
fn send_interrupt(child: &Child) {
    let _ = Command::new("kill")
        .arg("-INT")
        .arg(child.id().to_string())
        .status();
}

#[cfg(not(unix))]
fn send_interrupt(child: &mut Child) {
    let _ = child.kill();
}

fn take_child_output(child: Option<&mut Child>) -> (String, String) {
    let Some(child) = child else {
        return (String::new(), String::new());
    };

    let mut stdout = String::new();
    if let Some(mut pipe) = child.stdout.take() {
        let _ = pipe.read_to_string(&mut stdout);
    }

    let mut stderr = String::new();
    if let Some(mut pipe) = child.stderr.take() {
        let _ = pipe.read_to_string(&mut stderr);
    }

    (stdout, stderr)
}
