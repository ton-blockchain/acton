use std::net::TcpListener;
use std::process::{Child, Command, Output};
use std::thread;
use std::time::{Duration, Instant};

use crate::support::project::Project;
use acton_studio::StudioInfo;

mod reporting;
mod start;
mod test_runs;

const STUDIO_START_TIMEOUT: Duration = Duration::from_secs(10);
const STUDIO_STOP_TIMEOUT: Duration = Duration::from_secs(5);

struct StudioCliProcess {
    child: Option<Child>,
    url: String,
}

impl StudioCliProcess {
    fn start(project: &Project) -> Self {
        let (listener, port) = reserve_studio_port();
        drop(listener);
        let port_arg = port.to_string();
        let url = format!("http://127.0.0.1:{port}");
        let child = project
            .acton()
            .current_dir(project.path())
            .args(["studio", "start", "--port", &port_arg, "--no-open"])
            .spawn()
            .expect("Studio CLI process must start");
        let mut studio = Self {
            child: Some(child),
            url,
        };
        studio.wait_for_info();
        studio
    }

    fn id(&self) -> u32 {
        self.child
            .as_ref()
            .expect("Studio CLI process must be available")
            .id()
    }

    fn url(&self) -> &str {
        &self.url
    }

    fn wait_for_info(&mut self) -> StudioInfo {
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_millis(250))
            .build()
            .expect("Studio HTTP client must build");
        let deadline = Instant::now() + STUDIO_START_TIMEOUT;

        loop {
            if let Ok(response) = client.get(format!("{}/api/v1/info", self.url)).send()
                && response.status().is_success()
            {
                return response
                    .json()
                    .expect("Studio info response must contain valid JSON");
            }

            if self
                .child
                .as_mut()
                .expect("Studio CLI process must be available")
                .try_wait()
                .expect("Studio CLI process status must be readable")
                .is_some()
            {
                let output = self
                    .child
                    .take()
                    .expect("Studio CLI process must be available")
                    .wait_with_output()
                    .expect("Studio CLI output must be readable");
                panic!(
                    "Studio CLI exited before becoming ready\nstdout:\n{}\nstderr:\n{}",
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                );
            }

            assert!(
                Instant::now() < deadline,
                "Studio CLI did not become ready at {}",
                self.url
            );
            thread::sleep(Duration::from_millis(50));
        }
    }

    #[cfg(unix)]
    fn stop(mut self) -> Output {
        let child = self
            .child
            .as_mut()
            .expect("Studio CLI process must be available");
        let status = Command::new("kill")
            .arg("-INT")
            .arg(child.id().to_string())
            .status()
            .expect("SIGINT must be sent to Studio CLI");
        assert!(status.success(), "kill -INT failed with status {status}");

        let deadline = Instant::now() + STUDIO_STOP_TIMEOUT;
        while child
            .try_wait()
            .expect("Studio CLI process status must be readable")
            .is_none()
        {
            assert!(
                Instant::now() < deadline,
                "Studio CLI did not stop after SIGINT"
            );
            thread::sleep(Duration::from_millis(50));
        }

        self.child
            .take()
            .expect("Studio CLI process must be available")
            .wait_with_output()
            .expect("Studio CLI output must be readable")
    }
}

impl Drop for StudioCliProcess {
    fn drop(&mut self) {
        if let Some(child) = self.child.as_mut() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

fn reserve_studio_port() -> (TcpListener, u16) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("Studio test port must be reserved");
    let port = listener
        .local_addr()
        .expect("Reserved Studio TCP port has no address")
        .port();
    (listener, port)
}
