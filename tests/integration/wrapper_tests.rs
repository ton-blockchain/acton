use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;
use serde_json::Value;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

const SIMPLE_CONTRACT: &str = r"
fun onInternalMessage(in: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
";

#[cfg(unix)]
const FAKE_TYPESCRIPT_GENERATOR: &str = r#"#!/bin/sh
set -eu

if [ "${1:-}" = "--yes" ]; then
    shift
fi

if [ "${1:-}" != "gen-typescript-from-tolk" ] && [ "${1:-}" != "gen-typescript-from-tolk-dev" ]; then
    echo "unexpected package: ${1:-}" >&2
    exit 1
fi

printf '%s' "${2:-}" > "$ACTON_TS_WRAPPER_CAPTURE"
printf '%s\n' '// generated ts wrapper' 'export const marker = "ts";'
"#;

#[cfg(unix)]
fn make_typescript_wrapper_project(name: &str) -> crate::support::project::Project {
    ProjectBuilder::new(name)
        .contract("my_contract", SIMPLE_CONTRACT)
        .raw_file("bin/npx", FAKE_TYPESCRIPT_GENERATOR)
        .build()
}

#[cfg(unix)]
fn setup_fake_typescript_generator(project_root: &Path) -> (PathBuf, String) {
    use std::os::unix::fs::PermissionsExt;

    let npx_path = project_root.join("bin/npx");
    let mut permissions = fs::metadata(&npx_path).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&npx_path, permissions).unwrap();

    let capture_path = project_root.join("typescript-abi.json");
    let path_env = format!(
        "{}:{}",
        project_root.join("bin").display(),
        env::var("PATH").unwrap_or_default()
    );

    (capture_path, path_env)
}

#[test]
fn test_wrapper_generation_defaults() {
    let project = ProjectBuilder::new("wrapper_simple")
        .contract("my_contract", SIMPLE_CONTRACT)
        .build();

    let output = project
        .acton()
        .wrapper("my_contract")
        .generate_test_stub()
        .run()
        .success();

    output
        .assert_contains("Generated")
        .assert_file_snapshot_matches(
            project
                .path()
                .join("wrappers/MyContract.gen.tolk")
                .to_str()
                .expect(""),
            "integration/snapshots/wrapper/test_wrapper_generation_defaults/wrapper.tolk.txt",
        )
        .assert_file_snapshot_matches(
            project
                .path()
                .join("tests/my_contract.test.tolk")
                .to_str()
                .expect(""),
            "integration/snapshots/wrapper/test_wrapper_generation_defaults/test.tolk.txt",
        );
}

#[test]
fn test_wrapper_generation_without_test_stub() {
    let project = ProjectBuilder::new("wrapper_simple")
        .contract("my_contract", SIMPLE_CONTRACT)
        .build();

    let output = project.acton().wrapper("my_contract").run().success();

    output
        .assert_contains("Generated")
        .assert_file_snapshot_matches(
            project
                .path()
                .join("wrappers/MyContract.gen.tolk")
                .to_str()
                .expect(""),
            "integration/snapshots/wrapper/test_wrapper_generation_without_test_stub/wrapper.tolk.txt",
        );

    assert!(
        !project.path().join("tests/my_contract.test.tolk").exists(),
        "Test file should not exist"
    );
}

#[test]
fn test_wrapper_generation_uses_tolk_config_defaults() {
    let project = ProjectBuilder::new("wrapper_tolk_config_defaults")
        .contract("my_contract", SIMPLE_CONTRACT)
        .with_wrappers_tolk_output_dir("tests/generated-wrappers")
        .with_wrappers_tolk_generate_test(true)
        .with_wrappers_tolk_test_output_dir("tests/generated-tests")
        .build();

    let output = project.acton().wrapper("my_contract").run().success();

    output
        .assert_contains("Generated")
        .assert_contains("tests/generated-wrappers/MyContract.gen.tolk")
        .assert_contains("tests/generated-tests/my_contract.test.tolk");

    assert!(
        project
            .path()
            .join("tests/generated-wrappers/MyContract.gen.tolk")
            .exists()
    );
    assert!(
        project
            .path()
            .join("tests/generated-tests/my_contract.test.tolk")
            .exists()
    );
}

#[test]
fn test_wrapper_generation_test_output_dir_flag() {
    let project = ProjectBuilder::new("wrapper_test_output_dir_flag")
        .contract("my_contract", SIMPLE_CONTRACT)
        .build();

    let output = project
        .acton()
        .wrapper("my_contract")
        .generate_test_stub()
        .test_output_dir("generated-tests")
        .run()
        .success();

    output
        .assert_contains("Generated")
        .assert_contains("generated-tests/my_contract.test.tolk");

    assert!(
        project
            .path()
            .join("generated-tests/my_contract.test.tolk")
            .exists()
    );
}

#[cfg(unix)]
#[test]
fn test_wrapper_generation_typescript_defaults_to_wrapper_ts_dir() {
    let project = make_typescript_wrapper_project("wrapper_typescript");
    let (capture_path, path_env) = setup_fake_typescript_generator(project.path());

    let output = project
        .acton()
        .wrapper("my_contract")
        .generate_typescript_wrapper()
        .env("PATH", &path_env)
        .env(
            "ACTON_TS_WRAPPER_CAPTURE",
            capture_path.to_str().expect("capture path"),
        )
        .run()
        .success();

    output
        .assert_contains("Generated")
        .assert_contains("wrappers-ts/MyContract.gen.ts");

    assert_eq!(
        fs::read_to_string(project.path().join("wrappers-ts/MyContract.gen.ts")).unwrap(),
        "// generated ts wrapper\nexport const marker = \"ts\";\n"
    );

    let abi_json: Value = serde_json::from_str(&fs::read_to_string(&capture_path).unwrap())
        .expect("captured ABI JSON should be valid");
    assert_eq!(abi_json["contract_name"], "MyContract");
    assert_eq!(abi_json["compiler_name"], "tolk");
    assert!(
        abi_json["codeBoc64"]
            .as_str()
            .is_some_and(|value| !value.is_empty())
    );
}

#[cfg(unix)]
#[test]
fn test_wrapper_generation_typescript_uses_config_output_dir_relative_to_project_root() {
    let project = ProjectBuilder::new("wrapper_typescript_config_output_dir")
        .contract("my_contract", SIMPLE_CONTRACT)
        .with_wrappers_typescript_output_dir("./wrappers-ts")
        .raw_file("bin/npx", FAKE_TYPESCRIPT_GENERATOR)
        .build();
    let (capture_path, path_env) = setup_fake_typescript_generator(project.path());
    let project_root = project.path().display().to_string();

    let output = project
        .acton()
        .arg("--project-root")
        .arg(&project_root)
        .wrapper("my_contract")
        .generate_typescript_wrapper()
        .env("PATH", &path_env)
        .env(
            "ACTON_TS_WRAPPER_CAPTURE",
            capture_path.to_str().expect("capture path"),
        )
        .current_dir(project.path().parent().expect("project parent"))
        .run()
        .success();

    output
        .assert_contains("Generated")
        .assert_contains("wrappers-ts/MyContract.gen.ts");

    assert_eq!(
        fs::read_to_string(project.path().join("wrappers-ts/MyContract.gen.ts")).unwrap(),
        "// generated ts wrapper\nexport const marker = \"ts\";\n"
    );
}

#[cfg(unix)]
#[test]
fn test_wrapper_generation_typescript_output_dir_flag_overrides_config() {
    let project = ProjectBuilder::new("wrapper_typescript_output_dir_flag")
        .contract("my_contract", SIMPLE_CONTRACT)
        .with_wrappers_typescript_output_dir("./wrappers-config")
        .raw_file("bin/npx", FAKE_TYPESCRIPT_GENERATOR)
        .build();
    let (capture_path, path_env) = setup_fake_typescript_generator(project.path());

    let output = project
        .acton()
        .wrapper("my_contract")
        .generate_typescript_wrapper()
        .wrapper_output_dir("wrappers-cli")
        .env("PATH", &path_env)
        .env(
            "ACTON_TS_WRAPPER_CAPTURE",
            capture_path.to_str().expect("capture path"),
        )
        .run()
        .success();

    output
        .assert_contains("Generated")
        .assert_contains("wrappers-cli/MyContract.gen.ts");

    assert_eq!(
        fs::read_to_string(project.path().join("wrappers-cli/MyContract.gen.ts")).unwrap(),
        "// generated ts wrapper\nexport const marker = \"ts\";\n"
    );
    assert!(
        !project
            .path()
            .join("wrappers-config/MyContract.gen.ts")
            .exists(),
        "CLI output dir should override config output dir"
    );
}

#[cfg(unix)]
#[test]
fn test_wrapper_generation_typescript_ignores_tolk_test_defaults() {
    let project = ProjectBuilder::new("wrapper_typescript_ignores_tolk_defaults")
        .contract("my_contract", SIMPLE_CONTRACT)
        .with_wrappers_tolk_generate_test(true)
        .with_wrappers_tolk_test_output_dir("tests/generated-tests")
        .with_wrappers_typescript_output_dir("./wrappers-ts")
        .raw_file("bin/npx", FAKE_TYPESCRIPT_GENERATOR)
        .build();
    let (capture_path, path_env) = setup_fake_typescript_generator(project.path());

    let output = project
        .acton()
        .wrapper("my_contract")
        .generate_typescript_wrapper()
        .env("PATH", &path_env)
        .env(
            "ACTON_TS_WRAPPER_CAPTURE",
            capture_path.to_str().expect("capture path"),
        )
        .run()
        .success();

    output
        .assert_contains("Generated")
        .assert_contains("wrappers-ts/MyContract.gen.ts");

    assert!(
        project
            .path()
            .join("wrappers-ts/MyContract.gen.ts")
            .exists()
    );
    assert!(
        !project
            .path()
            .join("tests/generated-tests/my_contract.test.tolk")
            .exists(),
        "Tolk test defaults should be ignored in TypeScript mode"
    );
}

#[test]
fn test_wrapper_generation_typescript_conflicts_with_test_stub() {
    let project = ProjectBuilder::new("wrapper_typescript_conflict")
        .contract("my_contract", SIMPLE_CONTRACT)
        .build();

    project
        .acton()
        .wrapper("my_contract")
        .generate_typescript_wrapper()
        .generate_test_stub()
        .run()
        .failure()
        .assert_stderr_contains("cannot be used with '--test'");
}

#[test]
fn test_wrapper_generation_from_jetton_template_passes_fmt_check() {
    let workspace = ProjectBuilder::new("wrapper_jetton_template")
        .without_acton_toml()
        .build();

    let generated_project_name = "generated-jetton";
    let generated_project_path = workspace.path().join(generated_project_name);
    let generated_project_path_str = generated_project_path.display().to_string();

    workspace
        .acton()
        .arg("new")
        .arg(&generated_project_path_str)
        .arg("--name")
        .arg(generated_project_name)
        .arg("--description")
        .arg("Jetton wrapper generation fmt check")
        .arg("--template")
        .arg("jetton")
        .arg("--license")
        .arg("MIT")
        .current_dir(workspace.path())
        .run()
        .success();

    assert!(generated_project_path.join("Acton.toml").exists());

    workspace
        .acton()
        .current_dir(&generated_project_path)
        .arg("build")
        .run()
        .success();

    let tests_dir = generated_project_path.join("tests");
    if tests_dir.exists() {
        fs::remove_dir_all(&tests_dir).expect("Failed to remove template tests directory");
    }
    fs::create_dir_all(generated_project_path.join("wrappers"))
        .expect("Failed to recreate wrappers directory");

    let minter_output = workspace
        .acton()
        .arg("--project-root")
        .arg(&generated_project_path_str)
        .wrapper("JettonMinter")
        .generate_test_stub()
        .current_dir(workspace.path())
        .run()
        .success();
    minter_output.assert_contains("Generated");

    let wallet_output = workspace
        .acton()
        .arg("--project-root")
        .arg(&generated_project_path_str)
        .wrapper("JettonWallet")
        .generate_test_stub()
        .current_dir(workspace.path())
        .run()
        .success();
    wallet_output.assert_contains("Generated");

    assert!(
        generated_project_path
            .join("wrappers/JettonMinter.gen.tolk")
            .exists()
    );
    assert!(
        generated_project_path
            .join("wrappers/JettonWallet.gen.tolk")
            .exists()
    );
    assert!(
        generated_project_path
            .join("tests/JettonMinter.test.tolk")
            .exists()
    );
    assert!(
        generated_project_path
            .join("tests/JettonWallet.test.tolk")
            .exists()
    );

    wallet_output
        .assert_file_snapshot_matches(
            generated_project_path
                .join("wrappers/JettonMinter.gen.tolk")
                .to_str()
                .expect(""),
            "integration/snapshots/wrapper/test_wrapper_generation_from_jetton_template_passes_fmt_check/jetton_minter_wrapper.tolk.txt",
        )
        .assert_file_snapshot_matches(
            generated_project_path
                .join("wrappers/JettonWallet.gen.tolk")
                .to_str()
                .expect(""),
            "integration/snapshots/wrapper/test_wrapper_generation_from_jetton_template_passes_fmt_check/jetton_wallet_wrapper.tolk.txt",
        );

    workspace
        .acton()
        .arg("--project-root")
        .arg(&generated_project_path_str)
        .fmt()
        .arg("--check")
        .current_dir(workspace.path())
        .run()
        .success();
}

#[test]
fn test_wrapper_generation_with_types_and_storage_in_the_same_file() {
    let project = ProjectBuilder::new("wrapper_simple")
        .contract(
            "my_contract",
            r#"
                import "types"

                contract MyContract {
                    storage: Storage
                    incomingMessages: AllowedMessage
                }

                fun onInternalMessage(in: InMessage) {
                    val msg = lazy AllowedMessage.fromSlice(in.body);

                    match (msg) {
                        Increment => {}
                        Decrement => {}
                        else => {}
                    }
                }
            "#,
        )
        .file(
            "contracts/types",
            r"
                struct Storage {
                    id: uint32
                    counter: uint32
                }

                fun Storage.load(): Storage {
                    return Storage.fromCell(contract.getData());
                }

                fun Storage.save(self) {
                    contract.setData(self.toCell());
                }

                struct (0x00000001) Increment {
                    value: int32
                }

                struct (0x00000002) Decrement {
                    value: int32
                }

                type AllowedMessage = Increment | Decrement;
            ",
        )
        .build();

    let output = project
        .acton()
        .wrapper("my_contract")
        .generate_test_stub()
        .run()
        .success();

    output
        .assert_contains("Generated")
        .assert_file_snapshot_matches(
            project
                .path()
                .join("wrappers/MyContract.gen.tolk")
                .to_str()
                .expect(""),
            "integration/snapshots/wrapper/test_wrapper_generation_with_types_and_storage_in_the_same_file/wrapper.tolk.txt",
        )
        .assert_file_snapshot_matches(
            project
                .path()
                .join("tests/my_contract.test.tolk")
                .to_str()
                .expect(""),
            "integration/snapshots/wrapper/test_wrapper_generation_with_types_and_storage_in_the_same_file/test.tolk.txt",
        );
}

#[test]
fn test_wrapper_generation_with_several_storages() {
    let project = ProjectBuilder::new("wrapper_simple")
        .contract(
            "my_contract",
            r#"
                import "storage"

                contract MyContract {
                    storage: FirstStorage
                }

                fun onInternalMessage(in: InMessage) {}
                fun onBouncedMessage(_: InMessageBounced) {}
            "#,
        )
        .file(
            "contracts/storage",
            r"
                struct FirstStorage {
                    id: uint32
                    counter: uint32
                }

                fun FirstStorage.load() {
                    return FirstStorage.fromCell(contract.getData());
                }

                fun FirstStorage.save(self) {
                    contract.setData(self.toCell());
                }

                struct SecondStorage {
                    id: uint32
                    counter: uint32
                }

                fun SecondStorage.load() {
                    return SecondStorage.fromCell(contract.getData());
                }

                fun SecondStorage.save(self) {
                    contract.setData(self.toCell());
                }
            ",
        )
        .build();

    let output = project.acton().wrapper("my_contract").run().success();

    output
        .assert_contains("Generated")
        .assert_file_snapshot_matches(
            project
                .path()
                .join("wrappers/MyContract.gen.tolk")
                .to_str()
                .expect(""),
            "integration/snapshots/wrapper/test_wrapper_generation_with_several_storages/first_wrapper.tolk.txt",
        );
}

#[test]
fn test_wrapper_generation_with_typed_cell_field_in_storage() {
    let project = ProjectBuilder::new("wrapper_types")
        .contract(
            "my_contract",
            r#"
                import "storage"
                import "types"

                contract MyContract {
                    storage: Storage
                    incomingMessages: AllowedMessage
                }

                fun onInternalMessage(in: InMessage) {
                    val msg = lazy AllowedMessage.fromSlice(in.body);

                    match (msg) {
                        Increment => {}
                        Decrement => {}
                        else => {}
                    }
                }
            "#,
        )
        .file(
            "contracts/storage",
            r"
                struct Storage {
                    id: uint32
                    counter: Cell<uint32>
                }

                fun Storage.load(): Storage {
                    return Storage.fromCell(contract.getData());
                }

                fun Storage.save(self) {
                    contract.setData(self.toCell());
                }
            ",
        )
        .file(
            "contracts/types",
            r"
                struct (0x00000001) Increment {
                    value: int32
                }

                struct (0x00000002) Decrement {
                    value: int32
                }

                type AllowedMessage = Increment | Decrement;
            ",
        )
        .build();

    project
        .acton()
        .wrapper("my_contract")
        .generate_test_stub()
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/wrapper/test_wrapper_generation_with_typed_cell_field_in_storage/output.txt",
        )
        .assert_file_snapshot_matches(
            project
                .path()
                .join("wrappers/MyContract.gen.tolk")
                .to_str()
                .expect(""),
            "integration/snapshots/wrapper/test_wrapper_generation_with_typed_cell_field_in_storage/wrapper.tolk.txt",
        ).assert_file_snapshot_matches(
            project
                .path()
                .join("tests/my_contract.test.tolk")
                .to_str()
                .expect(""),
            "integration/snapshots/wrapper/test_wrapper_generation_with_typed_cell_field_in_storage/test.tolk.txt",
        );
}

#[test]
fn test_wrapper_generation_with_typed_cell_field() {
    let project = ProjectBuilder::new("wrapper_types")
        .contract(
            "my_contract",
            r#"
                import "storage"
                import "types"

                contract MyContract {
                    storage: Storage
                    incomingMessages: AllowedMessage
                }

                fun onInternalMessage(in: InMessage) {
                    val msg = lazy AllowedMessage.fromSlice(in.body);

                    match (msg) {
                        Increment => {}
                        Decrement => {}
                        else => {}
                    }
                }
            "#,
        )
        .file(
            "contracts/storage",
            r"
                struct Storage {
                    id: uint32
                    counter: uint32
                }

                fun Storage.load(): Storage {
                    return Storage.fromCell(contract.getData());
                }

                fun Storage.save(self) {
                    contract.setData(self.toCell());
                }
            ",
        )
        .file(
            "contracts/types",
            r"
                struct (0x00000001) Increment {
                    value: Cell<int32>
                }

                struct (0x00000002) Decrement {
                    value: int32
                }

                type AllowedMessage = Increment | Decrement;
            ",
        )
        .build();

    project
        .acton()
        .wrapper("my_contract")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/wrapper/test_wrapper_generation_with_typed_cell_field/output.txt",
        )
        .assert_file_snapshot_matches(
            project
                .path()
                .join("wrappers/MyContract.gen.tolk")
                .to_str()
                .expect(""),
            "integration/snapshots/wrapper/test_wrapper_generation_with_typed_cell_field/wrapper.tolk.txt",
        );
}

#[test]
fn test_wrapper_generation_with_typed_cell_param() {
    let project = ProjectBuilder::new("wrapper_types")
        .contract(
            "my_contract",
            r#"
                import "storage"
                import "types"

                contract MyContract {
                    storage: Storage
                    incomingMessages: AllowedMessage
                }

                fun onInternalMessage(in: InMessage) {
                    val msg = lazy AllowedMessage.fromSlice(in.body);

                    match (msg) {
                        Increment => {}
                        Decrement => {}
                        else => {}
                    }
                }

                get fun currentCounter(value: Cell<int32>) {}
            "#,
        )
        .file(
            "contracts/storage",
            r"
                struct Storage {
                    id: uint32
                    counter: uint32
                }

                fun Storage.load(): Storage {
                    return Storage.fromCell(contract.getData());
                }

                fun Storage.save(self) {
                    contract.setData(self.toCell());
                }
            ",
        )
        .file(
            "contracts/types",
            r"
                struct (0x00000001) Increment {
                    value: int32
                }

                struct (0x00000002) Decrement {
                    value: int32
                }

                type AllowedMessage = Increment | Decrement;
            ",
        )
        .build();

    project
        .acton()
        .wrapper("my_contract")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/wrapper/test_wrapper_generation_with_typed_cell_param/output.txt",
        )
        .assert_file_snapshot_matches(
            project
                .path()
                .join("wrappers/MyContract.gen.tolk")
                .to_str()
                .expect(""),
            "integration/snapshots/wrapper/test_wrapper_generation_with_typed_cell_param/wrapper.tolk.txt",
        );
}

#[test]
fn test_wrapper_generation_with_snake_case_getters() {
    let project = ProjectBuilder::new("wrapper_getters")
        .contract(
            "my_contract",
            r"
                contract MyContract {}

                fun onInternalMessage(_in: InMessage) {}

                get fun is_allowed(): bool {
                    return true;
                }

                get fun get_total_supply(owner_address: address): int {
                    return 0;
                }
            ",
        )
        .build();

    project
        .acton()
        .wrapper("my_contract")
        .run()
        .success()
        .assert_file_snapshot_matches(
            project
                .path()
                .join("wrappers/MyContract.gen.tolk")
                .to_str()
                .expect(""),
            "integration/snapshots/wrapper/test_wrapper_generation_with_snake_case_getters/wrapper.tolk.txt",
        );
}

#[test]
fn test_wrapper_custom_output() {
    let project = ProjectBuilder::new("wrapper_custom")
        .contract("my_contract", SIMPLE_CONTRACT)
        .build();

    let output = project
        .acton()
        .wrapper("my_contract")
        .generate_test_stub()
        .wrapper_output("custom/wrapper.tolk")
        .test_output("custom/test.tolk")
        .run()
        .success();

    output
        .assert_contains("Generated")
        .assert_file_snapshot_matches(
            project
                .path()
                .join("custom/wrapper.tolk")
                .to_str()
                .expect(""),
            "integration/snapshots/wrapper/test_wrapper_custom_output/wrapper.tolk.txt",
        )
        .assert_file_snapshot_matches(
            project.path().join("custom/test.tolk").to_str().expect(""),
            "integration/snapshots/wrapper/test_wrapper_custom_output/test.tolk.txt",
        );
}

#[test]
fn test_wrapper_output_dir_places_wrapper_in_directory() {
    let project = ProjectBuilder::new("wrapper_output_dir")
        .contract("my_contract", SIMPLE_CONTRACT)
        .build();

    let output = project
        .acton()
        .wrapper("my_contract")
        .generate_test_stub()
        .wrapper_output_dir("custom")
        .run()
        .success();

    output
        .assert_contains("Generated")
        .assert_contains("custom/MyContract.gen.tolk")
        .assert_file_snapshot_matches(
            project
                .path()
                .join("custom/MyContract.gen.tolk")
                .to_str()
                .expect(""),
            "integration/snapshots/wrapper/test_wrapper_generation_without_test_stub/wrapper.tolk.txt",
        );

    let test_code = fs::read_to_string(project.path().join("tests/my_contract.test.tolk")).unwrap();
    assert!(
        test_code.contains("import \"../custom/MyContract.gen\""),
        "test stub should import wrapper from custom directory:\n{test_code}"
    );
}

#[test]
fn test_wrapper_custom_output2() {
    let project = ProjectBuilder::new("wrapper_custom")
        .contract("my_contract", SIMPLE_CONTRACT)
        .build();

    let output = project
        .acton()
        .wrapper("my_contract")
        .generate_test_stub()
        .wrapper_output("custom/other/nested/wrapper.tolk")
        .test_output("custom/nested/other/test.tolk")
        .run()
        .success();

    output
        .assert_contains("Generated")
        .assert_file_snapshot_matches(
            project
                .path()
                .join("custom/other/nested/wrapper.tolk")
                .to_str()
                .expect(""),
            "integration/snapshots/wrapper/test_wrapper_custom_output2/wrapper.tolk.txt",
        )
        .assert_file_snapshot_matches(
            project
                .path()
                .join("custom/nested/other/test.tolk")
                .to_str()
                .expect(""),
            "integration/snapshots/wrapper/test_wrapper_custom_output2/test.tolk.txt",
        );
}

#[test]
fn test_with_several_files_contract() {
    let project = ProjectBuilder::new("wrapper_types")
        .contract(
            "my_contract",
            r#"
                import "storage"
                import "types"
                import "types_other"

                contract MyContract {
                    storage: Storage
                    incomingMessages: AllowedMessage
                }

                fun onInternalMessage(in: InMessage) {
                    val msg = lazy AllowedMessage.fromSlice(in.body);

                    match (msg) {
                        Increment => {}
                        Decrement => {}
                        else => {}
                    }
                }
            "#,
        )
        .file(
            "contracts/storage",
            r"
                struct Storage {
                    id: uint32
                    counter: uint32
                }

                fun Storage.load(): Storage {
                    return Storage.fromCell(contract.getData());
                }

                fun Storage.save(self) {
                    contract.setData(self.toCell());
                }
            ",
        )
        .file(
            "contracts/types",
            r#"
                import "types_other"

                struct (0x00000001) Increment {
                    value: int32
                }

                type AllowedMessage = Increment | Decrement;
            "#,
        )
        .file(
            "contracts/types_other",
            r"
                struct (0x00000002) Decrement {
                    value: int32
                }
            ",
        )
        .build();

    project
        .acton()
        .wrapper("my_contract")
        .generate_test_stub()
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/wrapper/test_with_several_files_contract/output.txt",
        )
        .assert_file_snapshot_matches(
            project
                .path()
                .join("wrappers/MyContract.gen.tolk")
                .to_str()
                .expect(""),
            "integration/snapshots/wrapper/test_with_several_files_contract/wrapper.tolk.txt",
        )
        .assert_file_snapshot_matches(
            project
                .path()
                .join("tests/my_contract.test.tolk")
                .to_str()
                .expect(""),
            "integration/snapshots/wrapper/test_with_several_files_contract/test.tolk.txt",
        );
}

#[test]
fn test_wrapper_with_storage_in_contract() {
    let project = ProjectBuilder::new("wrapper_types")
        .contract(
            "my_contract",
            r"
                struct Storage {
                    some: int32
                }

                contract MyContract {
                    storage: Storage
                }

                fun onInternalMessage(in: InMessage) {}
            ",
        )
        .build();

    project
        .acton()
        .wrapper("my_contract")
        .generate_test_stub()
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/wrapper/test_wrapper_with_storage_in_contract/output.txt",
        )
        .assert_file_snapshot_matches(
            project
                .path()
                .join("wrappers/MyContract.gen.tolk")
                .to_str()
                .expect(""),
            "integration/snapshots/wrapper/test_wrapper_with_storage_in_contract/wrapper.tolk.txt",
        )
        .assert_file_snapshot_matches(
            project
                .path()
                .join("tests/my_contract.test.tolk")
                .to_str()
                .expect(""),
            "integration/snapshots/wrapper/test_wrapper_with_storage_in_contract/test.tolk.txt",
        );
}

#[test]
fn test_wrapper_with_message_in_contract() {
    let project = ProjectBuilder::new("wrapper_types")
        .contract(
            "my_contract",
            r"
                struct (0x00000001) Increment {
                    value: int32
                }

                contract MyContract {
                    incomingMessages: Increment
                }

                fun onInternalMessage(in: InMessage) {}
            ",
        )
        .build();

    project
        .acton()
        .wrapper("my_contract")
        .generate_test_stub()
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/wrapper/test_wrapper_with_message_in_contract/output.txt",
        )
        .assert_file_snapshot_matches(
            project
                .path()
                .join("wrappers/MyContract.gen.tolk")
                .to_str()
                .expect(""),
            "integration/snapshots/wrapper/test_wrapper_with_message_in_contract/wrapper.tolk.txt",
        )
        .assert_file_snapshot_matches(
            project
                .path()
                .join("tests/my_contract.test.tolk")
                .to_str()
                .expect(""),
            "integration/snapshots/wrapper/test_wrapper_with_message_in_contract/test.tolk.txt",
        );
}

#[test]
fn test_generated_wrapper_test_runs_with_contract_local_types() {
    let workspace = ProjectBuilder::new("wrapper_types_runtime")
        .without_acton_toml()
        .build();

    let generated_project_name = "generated-counter";
    let generated_project_path = workspace.path().join(generated_project_name);
    let generated_project_path_str = generated_project_path.display().to_string();

    workspace
        .acton()
        .arg("new")
        .arg(&generated_project_path_str)
        .arg("--name")
        .arg(generated_project_name)
        .arg("--description")
        .arg("Wrapper runtime check")
        .arg("--template")
        .arg("counter")
        .arg("--license")
        .arg("MIT")
        .current_dir(workspace.path())
        .run()
        .success();

    let tests_dir = generated_project_path.join("tests");
    if tests_dir.exists() {
        fs::remove_dir_all(&tests_dir).expect("Failed to remove template tests directory");
    }
    fs::create_dir_all(generated_project_path.join("wrappers"))
        .expect("Failed to recreate wrappers directory");

    fs::write(
        generated_project_path.join("contracts/Counter.tolk"),
        r"
                struct Storage {
                    counter: uint32
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
            ",
    )
    .expect("Failed to write contract");

    workspace
        .acton()
        .arg("--project-root")
        .arg(&generated_project_path_str)
        .wrapper("Counter")
        .generate_test_stub()
        .env("ACTON_LOG_DIR", ".acton/logs")
        .current_dir(workspace.path())
        .run()
        .success()
        .assert_contains("Generated");

    workspace
        .acton()
        .arg("--project-root")
        .arg(&generated_project_path_str)
        .test()
        .env("ACTON_LOG_DIR", ".acton/logs")
        .current_dir(workspace.path())
        .run()
        .success()
        .assert_passed(1);
}

#[test]
fn test_wrapper_for_unknown_contract() {
    let project = ProjectBuilder::new("wrapper_simple")
        .contract("my_contract", SIMPLE_CONTRACT)
        .build();

    project
        .acton()
        .wrapper("unknown_contract")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/wrapper/test_wrapper_for_unknown_contract/stderr.txt",
        );
}

#[test]
fn test_wrapper_for_contract_without_file() {
    let project = ProjectBuilder::new("wrapper_simple")
        .contract("my_contract", SIMPLE_CONTRACT)
        .build();

    let contract_path = project.path().join("contracts/my_contract.tolk");
    fs::remove_file(contract_path).expect("should remove contract file");

    project
        .acton()
        .wrapper("my_contract")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/wrapper/test_wrapper_for_contract_without_file/stderr.txt",
        );
}

#[test]
fn test_wrapper_generation_with_mappings() {
    let project = ProjectBuilder::new("wrapper_mappings")
        .mapping("@core", "./libs/core")
        .file(
            "libs/core/types",
            r"
                struct (0x00000001) Increment {
                    value: int32
                }

                struct (0x00000002) Decrement {
                    value: int32
                }

                type AllowedMessage = Increment | Decrement;
            ",
        )
        .contract(
            "main",
            r#"
            import "@core/types"

            struct Storage {}

            contract Main {
                storage: Storage
                incomingMessages: AllowedMessage
            }

            fun onInternalMessage(in: InMessage) {
                val msg = lazy AllowedMessage.fromSlice(in.body);

                match (msg) {
                    Increment => {}
                    Decrement => {}
                    else => {}
                }
            }
            "#,
        )
        .build();

    project
        .acton()
        .wrapper("main")
        .generate_test_stub()
        .run()
        .success()
        .assert_file_snapshot_matches(
            project
                .path()
                .join("wrappers/Main.gen.tolk")
                .to_str()
                .expect(""),
            "integration/snapshots/wrapper/test_wrapper_generation_with_mappings/wrapper.tolk.txt",
        )
        .assert_file_snapshot_matches(
            project
                .path()
                .join("tests/main.test.tolk")
                .to_str()
                .expect(""),
            "integration/snapshots/wrapper/test_wrapper_generation_with_mappings/test.tolk.txt",
        );
}

#[test]
fn test_wrapper_generation_with_wrappers_mapping() {
    let project = ProjectBuilder::new("wrapper_wrappers_mapping")
        .mapping("wrappers", "tests/wrappers")
        .file(
            "contracts/types",
            r"
                struct Storage {
                    counter: int32
                }

                struct (0x00000001) Ping {
                    value: int32
                }

                type AllowedMessage = Ping;
            ",
        )
        .contract(
            "main",
            r#"
            import "types"

            contract Main {
                storage: Storage
                incomingMessages: AllowedMessage
            }

            fun onInternalMessage(in: InMessage) {
                val msg = lazy AllowedMessage.fromSlice(in.body);

                match (msg) {
                    Ping => {}
                    else => {}
                }
            }
            "#,
        )
        .build();

    project
        .acton()
        .wrapper("main")
        .generate_test_stub()
        .run()
        .success()
        .assert_file_snapshot_matches(
            project
                .path()
                .join("tests/wrappers/Main.gen.tolk")
                .to_str()
                .expect(""),
            "integration/snapshots/wrapper/test_wrapper_generation_with_wrappers_mapping/wrapper.tolk.txt",
        )
        .assert_file_snapshot_matches(
            project
                .path()
                .join("tests/main.test.tolk")
                .to_str()
                .expect(""),
            "integration/snapshots/wrapper/test_wrapper_generation_with_wrappers_mapping/test.tolk.txt",
        );
}

#[test]
fn test_wrapper_generation_prefers_specific_mapping() {
    let project = ProjectBuilder::new("wrapper_specific_mapping")
        .mapping("core", "libs")
        .mapping("core_sub", "libs/core")
        .file(
            "libs/core/types",
            r"
                struct (0x00000002) Pong {
                    value: int32
                }

                type AllowedMessage = Pong;
            ",
        )
        .contract(
            "main",
            r#"
            import "@core_sub/types"

            contract Main {
                incomingMessages: AllowedMessage
            }

            fun onInternalMessage(in: InMessage) {
                val msg = lazy AllowedMessage.fromSlice(in.body);

                match (msg) {
                    Pong => {}
                    else => {}
                }
            }
            "#,
        )
        .build();

    project
        .acton()
        .wrapper("main")
        .generate_test_stub()
        .run()
        .success()
        .assert_file_snapshot_matches(
            project
                .path()
                .join("wrappers/Main.gen.tolk")
                .to_str()
                .expect(""),
            "integration/snapshots/wrapper/test_wrapper_generation_prefers_specific_mapping/wrapper.tolk.txt",
        )
        .assert_file_snapshot_matches(
            project
                .path()
                .join("tests/main.test.tolk")
                .to_str()
                .expect(""),
            "integration/snapshots/wrapper/test_wrapper_generation_prefers_specific_mapping/test.tolk.txt",
        );
}

#[test]
fn test_wrapper_generation_with_import_mappings() {
    let project = ProjectBuilder::new("wrapper_import_mappings")
        .mapping("contracts", "contracts")
        .mapping("wrappers", "tests/wrappers")
        .file(
            "contracts/types",
            r"
                struct Storage {
                    counter: int32
                }

                struct (0x00000001) Increment {
                    value: int32
                }

                type AllowedMessage = Increment;
            ",
        )
        .contract(
            "my_contract",
            r#"
                import "@contracts/types"

                contract MyContract {
                    storage: Storage
                    incomingMessages: AllowedMessage
                }

                fun onInternalMessage(in: InMessage) {
                    val msg = lazy AllowedMessage.fromSlice(in.body);

                    match (msg) {
                        Increment => {}
                        else => {}
                    }
                }
            "#,
        )
        .build();

    project
        .acton()
        .wrapper("my_contract")
        .generate_test_stub()
        .run()
        .success()
        .assert_file_snapshot_matches(
            project
                .path()
                .join("tests/wrappers/MyContract.gen.tolk")
                .to_str()
                .expect(""),
            "integration/snapshots/wrapper/test_wrapper_generation_with_import_mappings/wrapper.tolk.txt",
        )
        .assert_file_snapshot_matches(
            project
                .path()
                .join("tests/my_contract.test.tolk")
                .to_str()
                .expect(""),
            "integration/snapshots/wrapper/test_wrapper_generation_with_import_mappings/test.tolk.txt",
        );
}

#[test]
fn test_wrapper_generation_with_conflicting_field_names() {
    let project = ProjectBuilder::new("wrapper_conflicts")
        .contract(
            "my_contract",
            r#"
                import "types"

                contract MyContract {
                    incomingMessages: AllowedMessage
                }

                fun onInternalMessage(in: InMessage) {
                    val msg = lazy AllowedMessage.fromSlice(in.body);

                    match (msg) {
                        MessageWithConflicts => {}
                        else => {}
                    }
                }
            "#,
        )
        .file(
            "contracts/types",
            r"
                struct (0x00000001) MessageWithConflicts {
                    from: address
                    config: int32
                    other: uint32
                }

                type AllowedMessage = MessageWithConflicts;
            ",
        )
        .build();

    let output = project.acton().wrapper("my_contract").run().success();

    output
        .assert_contains("Generated")
        .assert_file_snapshot_matches(
            project
                .path()
                .join("wrappers/MyContract.gen.tolk")
                .to_str()
                .expect(""),
            "integration/snapshots/wrapper/test_wrapper_generation_with_conflicting_field_names/wrapper.tolk.txt",
        );
}
