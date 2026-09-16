use crate::common::{acton_exe, assertion};
use crate::support::project::ProjectBuilder;
use expectrl::process::unix::WaitStatus;
use expectrl::{Eof, Expect};
use std::fmt::Write;
use std::process::Command;
use std::time::Duration;

const PROMPTS: &[(&str, &str)] = &[
    ("text", r#"prompt("First prompt", "", "default")"#),
    ("integer", r#"promptInt("First prompt", "42")"#),
    (
        "address",
        r#"promptAddress("First prompt", address("0:0000000000000000000000000000000000000000000000000000000000000000"))"#,
    ),
    ("select", r#"select("First prompt", ["one", "two"], 1)"#),
    ("confirm", r#"confirm("First prompt", true, "")"#),
    ("wallet", r#"promptWallet("First prompt")"#),
];

// Configuring two wallets opens the picker without sending any messages.
const WALLETS: &str = r#"
[wallets.first]
kind = "v4r2"
keys = { mnemonic = "cupboard match uphold miracle fog balance unknown region share hand trophy million toy narrow ability exchange first toast fresh maid report cram strong later" }

[wallets.second]
kind = "v4r2"
keys = { mnemonic = "section garden tomato dinner season dice renew length useful spin trade intact use universe what post spike keen mandate behind concert egg doll rug" }
"#;

fn script(prompt: &str) -> String {
    format!(
        r#"
import "../../lib/prompts"
import "../../lib/fs"

fun main() {{
    fs.writeString("started.txt", "started");
    try {{
        val _ = {prompt};
        fs.writeString("continued.txt", "continued");
    }} catch (_, _) {{
        fs.writeString("caught.txt", "caught");
    }}
    fs.writeString("next-prompt.txt", "next");
    val _ = prompt("Second prompt", "", "default");
}}
"#
    )
}

#[test]
fn cancelling_interactive_prompts_stops_the_script() {
    let mut outcomes = String::new();
    for (key, input) in [("ctrl-c", "\x03"), ("escape", "\x1b")] {
        for (name, prompt) in PROMPTS {
            let project = ProjectBuilder::new(&format!("cancel-{key}-{name}"))
                .script_file("prompt", &script(prompt))
                .build();
            let mut command = project.acton().script("scripts/prompt.tolk");
            if *name == "wallet" {
                std::fs::write(project.path().join("wallets.toml"), WALLETS).unwrap();
                command = command.verify_network("testnet");
            }
            let mut session = command
                .spawn_pty()
                .set_expect_timeout(Some(Duration::from_secs(10)));

            session.expect("First prompt");
            session.send(input).expect("failed to cancel prompt");
            if key == "escape" {
                session.expect("Prompt \"First prompt\": Operation was canceled by the user");
            }
            session.expect(Eof);
            let WaitStatus::Exited(_, code) = session.get_process().wait().unwrap() else {
                panic!("prompt process did not exit normally");
            };
            writeln!(
                outcomes,
                "{name} {key}: exit={code}, continued={}, caught={}, next_prompt={}",
                project.path().join("continued.txt").exists(),
                project.path().join("caught.txt").exists(),
                project.path().join("next-prompt.txt").exists(),
            )
            .unwrap();
        }
    }
    assertion().eq(
        outcomes,
        snapbox::file!["snapshots/prompts/interactive_cancellation.txt"],
    );
}

#[test]
fn terminal_write_errors_stop_interactive_prompts() {
    let mut outcomes = String::new();
    for (name, prompt) in PROMPTS {
        let project = ProjectBuilder::new(&format!("prompt-io-error-{name}"))
            .script_file("prompt", &script(prompt))
            .build();
        // Keep stdin on the terminal, but send stderr to a pipe without readers.
        // Open it read/write to avoid blocking, then close that reader before
        // starting Acton. Rust suppresses EBADF from read-only stderr, but it
        // propagates BrokenPipe so drawing the first prompt fails.
        let mut command = Command::new("sh");
        command
            .args([
                "-ec",
                r#"
mkfifo stderr.fifo
exec 3<>stderr.fifo
exec 2>stderr.fifo
exec 3>&-
exec "$@"
"#,
                "sh",
            ])
            .arg(acton_exe())
            .args(["script", "scripts/prompt.tolk"])
            .current_dir(project.path())
            .env("HOME", project.isolated_home())
            .env("ACTON_LOG_DIR", project.path().join(".acton-test-logs"));
        if *name == "wallet" {
            std::fs::write(project.path().join("wallets.toml"), WALLETS).unwrap();
            command.args(["--net", "testnet"]);
        }
        let mut session = expectrl::Session::spawn(command).unwrap();
        session.set_expect_timeout(Some(Duration::from_secs(10)));
        session
            .expect(Eof)
            .expect("script did not stop after I/O failure");
        let status = session.get_process().wait().unwrap();
        let WaitStatus::Exited(_, code) = status else {
            panic!("{name}: prompt process did not exit normally: {status:?}");
        };
        writeln!(
            outcomes,
            "{name}: exit={code}, started={}, continued={}, caught={}, next_prompt={}",
            project.path().join("started.txt").exists(),
            project.path().join("continued.txt").exists(),
            project.path().join("caught.txt").exists(),
            project.path().join("next-prompt.txt").exists(),
        )
        .unwrap();
    }
    assertion().eq(
        outcomes,
        snapbox::file!["snapshots/prompts/terminal_write_errors.txt"],
    );
}

#[test]
fn enter_accepts_interactive_defaults() {
    let project = ProjectBuilder::new("prompt-enter-defaults")
        .script_file(
            "defaults",
            r#"
import "../../lib/prompts"
import "../../lib/fs"
import "../../lib/fmt"

fun main() {
    val empty = select("Empty selection", []);
    val text = prompt("Text default", "", "Guest");
    val number = promptInt("Integer default", "42");
    val defaultAddress = address("0:0000000000000000000000000000000000000000000000000000000000000000");
    val recipient = promptAddress("Address default", defaultAddress);
    val choice = select("Selection default", ["one", "two"], 1);
    val confirmed = confirm("Confirmation default", true, "");
    fs.writeString("defaults.txt", format(
        "empty='{}'\n{}", empty,
        format(
            "text={}, integer={}, address_matches={}, choice={}, confirmed={}\n",
            text, number, recipient == defaultAddress, choice, confirmed,
        ),
    ));
}
"#,
        )
        .build();
    let mut session = project
        .acton()
        .script("scripts/defaults.tolk")
        .spawn_pty()
        .set_expect_timeout(Some(Duration::from_secs(10)));
    for label in [
        "Text default",
        "Integer default",
        "Address default",
        "Selection default",
        "Confirmation default",
    ] {
        session.expect(label);
        session.send_line("", "failed to accept default");
    }
    session.expect(Eof);
    session.assert_file_snapshot_matches(
        "defaults.txt",
        "integration/snapshots/prompts/interactive_defaults.txt",
    );
}
