use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;
use std::fmt::Write as _;
use std::fs;
use std::path::Path;

const ABI_CONTRACT: &str = r"
struct Storage {
    counter: int32
}

struct (0x00000001) Increment {
    value: int32
}

contract Counter {
    storage: Storage
    incomingMessages: Increment
}

fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
";

fn append_build_output_abi(project_root: &Path, output_abi: &str) {
    let acton_toml_path = project_root.join("Acton.toml");
    let mut acton_toml = fs::read_to_string(&acton_toml_path).expect("read Acton.toml");
    let _ = write!(acton_toml, "\n[build]\noutput-abi = \"{output_abi}\"\n");
    fs::write(acton_toml_path, acton_toml).expect("write Acton.toml with [build] section");
}

#[test]
fn build_writes_contract_abi_to_default_directory() {
    let project = ProjectBuilder::new("build-output-abi-default")
        .contract("counter", ABI_CONTRACT)
        .build();

    let output = project.acton().build().run().success();

    output
        .assert_snapshot_matches(
            "integration/snapshots/build/build_cmd_output_abi_tests/build_writes_contract_abi_to_default_directory.stdout.txt",
        )
        .assert_file_snapshot_matches(
            "build/abi/counter.json",
            "integration/snapshots/build/build_cmd_output_abi_tests/build_writes_contract_abi_to_default_directory.abi.json",
        );
}

#[test]
fn build_writes_contract_abi_to_configured_directory() {
    let project = ProjectBuilder::new("build-output-abi-config")
        .contract("counter", ABI_CONTRACT)
        .build();

    append_build_output_abi(project.path(), "custom abi");

    let output = project.acton().build().run().success();

    output.assert_file_snapshot_matches(
        "custom abi/counter.json",
        "integration/snapshots/build/build_cmd_output_abi_tests/build_writes_contract_abi_to_configured_directory.abi.json",
    );
}

#[test]
fn build_output_abi_cli_flag_overrides_configured_directory() {
    let project = ProjectBuilder::new("build-output-abi-cli-overrides-config")
        .contract("counter", ABI_CONTRACT)
        .build();

    append_build_output_abi(project.path(), "config/abi");

    let output = project
        .acton()
        .build()
        .with_output_abi("cli/abi")
        .run()
        .success();

    output.assert_file_snapshot_matches(
        "cli/abi/counter.json",
        "integration/snapshots/build/build_cmd_output_abi_tests/build_output_abi_cli_flag_overrides_configured_directory.abi.json",
    );
}
