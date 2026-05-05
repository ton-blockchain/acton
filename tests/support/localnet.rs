use crate::common::acton_exe;
use crate::support::project::{ActonCommand, Project};
use reqwest::blocking::Client;
use serde_json::Value;
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
    args: Vec<String>,
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
        Self {
            project,
            current_dir: project.path().to_path_buf(),
            port: find_available_port(),
            args: Vec::new(),
            ready_timeout: DEFAULT_READY_TIMEOUT,
        }
    }

    pub(crate) fn current_dir(mut self, path: impl AsRef<Path>) -> Self {
        self.current_dir = path.as_ref().to_path_buf();
        self
    }

    pub(crate) fn port(mut self, port: u16) -> Self {
        self.port = port;
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

    pub(crate) fn before_start<F>(self, configure: F) -> Self
    where
        F: FnOnce(ActonCommand) -> ActonCommand,
    {
        configure(self.project.acton()).run().success();
        self
    }

    pub(crate) fn start(self) -> LocalnetHandle {
        let mut cmd = Command::new(acton_exe());
        cmd.arg("localnet")
            .arg("start")
            .arg("--port")
            .arg(self.port.to_string());
        cmd.args(&self.args)
            .current_dir(&self.current_dir)
            .env("NO_COLOR", "1")
            .env("HOME", self.project.isolated_home())
            .env("USERPROFILE", self.project.isolated_home())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let child = cmd.spawn().unwrap_or_else(|e| {
            panic!(
                "Failed to start `acton localnet start --port {}`: {}",
                self.port, e
            )
        });

        let mut handle = LocalnetHandle {
            child: Some(child),
            port: self.port,
            base_url: format!("http://localhost:{}", self.port),
            client: Client::builder()
                .timeout(Duration::from_secs(5))
                .build()
                .expect("Failed to create HTTP client for localnet tests"),
        };

        match handle.wait_until_ready(self.ready_timeout) {
            Ok(base_url) => {
                handle.base_url = base_url;
            }
            Err(err) => {
                let logs = handle.terminate_and_collect_output();
                panic!(
                    "Localnet failed to become ready on port {}: {}\n{}",
                    self.port, err, logs
                );
            }
        }

        handle
    }
}

pub(crate) struct LocalnetHandle {
    child: Option<Child>,
    port: u16,
    base_url: String,
    client: Client,
}

#[allow(dead_code)]
impl LocalnetHandle {
    pub(crate) fn port(&self) -> u16 {
        self.port
    }

    pub(crate) fn base_url(&self) -> String {
        self.base_url.clone()
    }

    pub(crate) fn get_json(&self, path: &str) -> Value {
        let url = format!("{}{}", self.base_url(), normalize_path(path));
        let response = self
            .client
            .get(&url)
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
            .client
            .get(&url)
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

    pub(crate) fn post_json(&self, path: &str, payload: &Value) -> Value {
        let url = format!("{}{}", self.base_url(), normalize_path(path));
        let response = self
            .client
            .post(&url)
            .json(payload)
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
            .client
            .post(&url)
            .json(payload)
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

    pub(crate) fn stop(mut self) {
        self.terminate();
    }

    fn wait_until_ready(&mut self, timeout: Duration) -> Result<String, String> {
        let deadline = Instant::now() + timeout;
        let probe_urls = [
            format!("http://localhost:{}/api/v2/getMasterchainInfo", self.port),
            format!("http://127.0.0.1:{}/api/v2/getMasterchainInfo", self.port),
        ];

        loop {
            if let Some(status) = self
                .child_mut()
                .try_wait()
                .map_err(|e| format!("Failed to poll Localnet process: {e}"))?
            {
                return Err(format!("Localnet exited before ready with status {status}"));
            }

            for url in &probe_urls {
                if let Ok(response) = self.client.get(url).send()
                    && response.status().is_success()
                    && let Ok(json) = response.json::<Value>()
                    && json.get("ok").and_then(Value::as_bool) == Some(true)
                {
                    let base_url = url.trim_end_matches("/api/v2/getMasterchainInfo");
                    return Ok(base_url.to_string());
                }
            }

            if Instant::now() >= deadline {
                return Err(format!(
                    "Timed out waiting for readiness probe {}",
                    probe_urls.join(" or ")
                ));
            }

            thread::sleep(Duration::from_millis(100));
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

fn find_available_port() -> u16 {
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .expect("Failed to reserve an ephemeral port for localnet tests");
    listener
        .local_addr()
        .expect("Failed to read ephemeral port address")
        .port()
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
