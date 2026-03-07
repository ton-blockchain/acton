use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;
use std::fs;

const SIMPLE_CONTRACT: &str = r"
fun onInternalMessage(in: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
";

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
                .join("tests/wrappers/MyContract.tolk")
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
fn test_wrapper_generation_with_types_and_storage_in_the_same_file() {
    let project = ProjectBuilder::new("wrapper_simple")
        .contract(
            "my_contract",
            r#"
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
                    value: int
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
                .join("tests/wrappers/MyContract.tolk")
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
            ",
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
fn test_wrapper_generation_with_typed_cell_field_in_storage() {
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
                    value: int
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
                .join("tests/wrappers/MyContract.tolk")
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
                    value: int
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
                .join("tests/wrappers/MyContract.tolk")
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
                    value: int
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
                .join("tests/wrappers/MyContract.tolk")
                .to_str()
                .expect(""),
            "integration/snapshots/wrapper/test_wrapper_generation_with_typed_cell_param/wrapper.tolk.txt",
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
                    value: int
                }

                type AllowedMessage = Increment | Decrement;
            "#,
        )
        .file(
            "contracts/types_other",
            r"
                struct (0x00000002) Decrement {
                    value: int
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
                .join("tests/wrappers/MyContract.tolk")
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
                    some: int
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
                .join("tests/wrappers/MyContract.tolk")
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
                    value: int
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
                .join("tests/wrappers/MyContract.tolk")
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
            r#"
                struct (0x00000001) Increment {
                    value: int32
                }

                struct (0x00000002) Decrement {
                    value: int
                }

                type AllowedMessage = Increment | Decrement;
            "#,
        )
        .contract(
            "main",
            r#"
            import "@core/types"

            struct Storage {}

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
                .join("tests/wrappers/Main.tolk")
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
            r#"
                struct Storage {
                    counter: int32
                }

                struct (0x00000001) Ping {
                    value: int32
                }

                type AllowedMessage = Ping;
            "#,
        )
        .contract(
            "main",
            r#"
            import "types"

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
                .join("tests/wrappers/Main.tolk")
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
            r#"
                struct (0x00000002) Pong {
                    value: int
                }

                type AllowedMessage = Pong;
            "#,
        )
        .contract(
            "main",
            r#"
            import "@core_sub/types"

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
                .join("tests/wrappers/Main.tolk")
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
            r#"
                struct Storage {
                    counter: int32
                }

                struct (0x00000001) Increment {
                    value: int32
                }

                type AllowedMessage = Increment;
            "#,
        )
        .contract(
            "my_contract",
            r#"
                import "@contracts/types"

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
                .join("tests/wrappers/MyContract.tolk")
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
                    config: int
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
                .join("tests/wrappers/MyContract.tolk")
                .to_str()
                .expect(""),
            "integration/snapshots/wrapper/test_wrapper_generation_with_conflicting_field_names/wrapper.tolk.txt",
        );
}
