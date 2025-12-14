use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;
use std::fs;

const SIMPLE_CONTRACT: &str = r#"
fun onInternalMessage(in: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
"#;

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
                .join("tests/wrappers/MyContract.tolk")
                .to_str()
                .expect(""),
            "integration/snapshots/wrapper/test_wrapper_generation_defaults/wrapper.tolk.txt",
        )
        .assert_file_snapshot_matches(
            project
                .path()
                .join("tests/my_contract_test.tolk")
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
                .join("tests/wrappers/MyContract.tolk")
                .to_str()
                .expect(""),
            "integration/snapshots/wrapper/test_wrapper_generation_without_test_stub/wrapper.tolk.txt",
        );

    assert!(
        !project.path().join("tests/my_contract_test.tolk").exists(),
        "Test file should not exist"
    );
}

#[test]
fn test_wrapper_generation_with_several_storages() {
    let project = ProjectBuilder::new("wrapper_simple")
        .contract(
            "my_contract",
            r#"
                import "storage"

                fun onInternalMessage(in: InMessage) {}
                fun onBouncedMessage(_: InMessageBounced) {}
            "#,
        )
        .file(
            "contracts/storage",
            r#"
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
            "#,
        )
        .build();

    let output = project
        .acton()
        .wrapper("my_contract")
        .storage_struct("FirstStorage")
        .run()
        .success();

    output
        .assert_contains("Generated")
        .assert_file_snapshot_matches(
            project
                .path()
                .join("tests/wrappers/MyContract.tolk")
                .to_str()
                .expect(""),
            "integration/snapshots/wrapper/test_wrapper_generation_with_several_storages/first_wrapper.tolk.txt",
        );

    let output = project
        .acton()
        .wrapper("my_contract")
        .storage_struct("SecondStorage")
        .run()
        .success();

    output
        .assert_contains("Generated")
        .assert_file_snapshot_matches(
            project
                .path()
                .join("tests/wrappers/MyContract.tolk")
                .to_str()
                .expect(""),
            "integration/snapshots/wrapper/test_wrapper_generation_with_several_storages/second_wrapper.tolk.txt",
        );
}

#[test]
fn test_wrapper_generation_with_unknown_explicit_storage() {
    let project = ProjectBuilder::new("wrapper_simple")
        .contract(
            "my_contract",
            r#"
                import "storage"

                fun onInternalMessage(in: InMessage) {}
                fun onBouncedMessage(_: InMessageBounced) {}
            "#,
        )
        .file(
            "contracts/storage",
            r#"
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
            "#,
        )
        .build();

    project
        .acton()
        .wrapper("my_contract")
        .storage_struct("SomeStorage")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/wrapper/test_wrapper_generation_with_unknown_explicit_storage/stderr.txt",
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
            r#"
                struct Storage {
                    id: uint32
                    counter: uint32
                }

                fun Storage.load() {
                    return Storage.fromCell(contract.getData());
                }

                fun Storage.save(self) {
                    contract.setData(self.toCell());
                }
            "#,
        )
        .file(
            "contracts/types",
            r#"
                import "types_other"

                struct (0x00000001) Increment {
                    value: int
                }

                type AllowedMessage = Increment | Decrement;
            "#,
        )
        .file(
            "contracts/types_other",
            r#"
                struct (0x00000002) Decrement {
                    value: int
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
        .assert_snapshot_matches(
            "integration/snapshots/wrapper/test_with_several_files_contract/output.txt",
        )
        .assert_file_snapshot_matches(
            project
                .path()
                .join("tests/wrappers/MyContract.tolk")
                .to_str()
                .expect(""),
            "integration/snapshots/wrapper/test_with_several_files_contract/wrapper.tolk.txt",
        )
        .assert_file_snapshot_matches(
            project
                .path()
                .join("tests/my_contract_test.tolk")
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
            r#"
                struct Storage {
                    some: int
                }

                fun onInternalMessage(in: InMessage) {}
            "#,
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
                .join("tests/wrappers/MyContract.tolk")
                .to_str()
                .expect(""),
            "integration/snapshots/wrapper/test_wrapper_with_storage_in_contract/wrapper.tolk.txt",
        )
        .assert_file_snapshot_matches(
            project
                .path()
                .join("tests/my_contract_test.tolk")
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
            r#"
                struct (0x00000001) Increment {
                    value: int
                }

                fun onInternalMessage(in: InMessage) {}
            "#,
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
                .join("tests/wrappers/MyContract.tolk")
                .to_str()
                .expect(""),
            "integration/snapshots/wrapper/test_wrapper_with_message_in_contract/wrapper.tolk.txt",
        )
        .assert_file_snapshot_matches(
            project
                .path()
                .join("tests/my_contract_test.tolk")
                .to_str()
                .expect(""),
            "integration/snapshots/wrapper/test_wrapper_with_message_in_contract/test.tolk.txt",
        );
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
    fs::remove_file(contract_path.clone()).expect("should remove contract file");

    project
        .acton()
        .wrapper("my_contract")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/wrapper/test_wrapper_for_contract_without_file/stderr.txt",
        );
}
