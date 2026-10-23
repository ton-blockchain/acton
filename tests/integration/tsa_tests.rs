use crate::support::TestOutputExt;
use crate::support::project::{ActonCommand, Project, ProjectBuilder};
use sha2::{Digest, Sha256};
use std::io::{ErrorKind, Read, Write};
use std::net::TcpListener;
use std::thread;
use std::time::{Duration, Instant};

const SIMPLE_CONTRACT: &str =
    "fun onInternalMessage(_: InMessage) {}\nfun onBouncedMessage(_: InMessageBounced) {}\n";
const TEST_JAR: &[u8] = b"mock TSA JAR\n";
const SYMBOLIC_CHECKER: &str = r#"
import "@acton/tsa"

fun onInternalMessage() {
    tsa.sendSymbolicInternalMessage(0, 1);
    tsa.sendSymbolicExternalMessage(0, 2);

    val contract = tsa.contract(0);
    contract.sendSymbolicInternalMessage(3);
    contract.sendSymbolicExternalMessage(4);
}
"#;

fn tsa_project(name: &str) -> Project {
    ProjectBuilder::new(name)
        .contract("target", SIMPLE_CONTRACT)
        .raw_file("checker.boc", "checker")
        .build()
}

fn tsa_check(project: &Project) -> ActonCommand {
    let root = project.path().to_str().expect("UTF-8 test project path");
    let checker = project
        .path()
        .join("checker.boc")
        .to_str()
        .expect("UTF-8 checker path")
        .to_owned();

    project
        .acton()
        .args(["--project-root", root, "tsa", "check", "--checker"])
        .arg(&checker)
        .arg("target")
        .env_remove("ACTON_TSA_JAR")
}

fn jar_path(project: &Project) -> std::path::PathBuf {
    project.path().join(".acton/tsa/tsa-cli-v0.1.0-acton.1.jar")
}

#[test]
fn tsa_symbolic_message_helpers_compile() {
    let project = ProjectBuilder::new("tsa-symbolic-messages")
        .contract("checker", SYMBOLIC_CHECKER)
        .mapping("@acton", "../lib")
        .build();

    project
        .acton()
        .compile("contracts/checker.tolk")
        .run()
        .success()
        .assert_snapshot_matches("integration/snapshots/tsa/symbolic-message-compile.txt");
}

#[test]
fn tsa_check_requires_consent_for_missing_jar_without_tty() {
    let project = tsa_project("tsa-missing-jar");

    let output = tsa_check(&project).run().failure();

    output.assert_stderr_contains("TSA v0.1.0-acton.1 is not installed");
    output.assert_stderr_contains("--yes");
    output.assert_stderr_contains("--tsa-cli");
    output.assert_stderr_snapshot_matches("integration/snapshots/tsa/missing-jar.stderr.txt");
    assert!(
        !jar_path(&project).exists(),
        "TSA must not download without consent"
    );
}

#[test]
fn tsa_check_installs_verified_jar_with_yes() {
    let project = tsa_project("tsa-install-verified");
    let server = TestDownloadServer::spawn(TEST_JAR);
    let sha256 = hex::encode(Sha256::digest(TEST_JAR));

    let output = tsa_check(&project)
        .arg("--yes")
        .env("ACTON_TEST_TSA_DOWNLOAD_URL", &server.url)
        .env("ACTON_TEST_TSA_DOWNLOAD_SIZE", &TEST_JAR.len().to_string())
        .env("ACTON_TEST_TSA_DOWNLOAD_SHA256", &sha256)
        .run()
        .failure();
    server.finish();

    output.assert_contains("Installed TSA v0.1.0-acton.1");
    output.assert_file_snapshot_matches(
        ".acton/tsa/tsa-cli-v0.1.0-acton.1.jar",
        "integration/snapshots/tsa/verified-jar.txt",
    );

    // The test JAR is deliberately not executable; the second run proves the cached file
    // is reused without another prompt or network request.
    let repeated = tsa_check(&project).run().failure();
    repeated.assert_not_contains("Downloading TSA");
    repeated.assert_not_contains("TSA v0.1.0-acton.1 is not installed");
}

#[test]
fn tsa_check_rejects_invalid_download_without_installing_it() {
    let project = tsa_project("tsa-install-bad-checksum");
    let server = TestDownloadServer::spawn(TEST_JAR);

    let output = tsa_check(&project)
        .arg("--yes")
        .env("ACTON_TEST_TSA_DOWNLOAD_URL", &server.url)
        .env("ACTON_TEST_TSA_DOWNLOAD_SIZE", &TEST_JAR.len().to_string())
        .env("ACTON_TEST_TSA_DOWNLOAD_SHA256", &"0".repeat(64))
        .run()
        .failure();
    server.finish();

    output.assert_stderr_contains("TSA download checksum mismatch");
    assert!(
        !jar_path(&project).exists(),
        "A failed download must leave no installed JAR"
    );
}

struct TestDownloadServer {
    url: String,
    handle: thread::JoinHandle<()>,
}

impl TestDownloadServer {
    fn spawn(body: &[u8]) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("Bind TSA download server");
        listener
            .set_nonblocking(true)
            .expect("Configure TSA download server");
        let address = listener.local_addr().expect("Read TSA download address");
        let body = body.to_vec();
        let handle = thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(20);
            loop {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let mut request = [0_u8; 1024];
                        let request_length = stream
                            .read(&mut request)
                            .expect("Read TSA download request");
                        assert!(request_length > 0, "TSA download request was empty");
                        write!(
                            stream,
                            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                            body.len()
                        )
                        .expect("Write TSA download response headers");
                        stream
                            .write_all(&body)
                            .expect("Write TSA download response body");
                        return;
                    }
                    Err(error) if error.kind() == ErrorKind::WouldBlock => {
                        assert!(
                            Instant::now() < deadline,
                            "TSA download request did not arrive"
                        );
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(error) => panic!("TSA download server failed: {error}"),
                }
            }
        });

        Self {
            url: format!("http://{address}/tsa-cli.jar"),
            handle,
        }
    }

    fn finish(self) {
        self.handle.join().expect("TSA download server panicked");
    }
}
