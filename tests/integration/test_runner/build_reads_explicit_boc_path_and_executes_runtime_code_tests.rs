use crate::support::TestOutputExt;
use crate::support::fixtures::FixtureProject;
use crate::support::project::ProjectBuilder;
use std::fs;

const SIMPLE_RUNTIME_CONTRACT: &str = r"
fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}

get fun ping(): int {
    return 7;
}
";

const PRECOMPILED_MESSAGE_CONTRACT: &str = r"
struct (0x7e8764ef) IncreaseCounter {
    increaseBy: uint32
}

contract Precompiled {
    incomingMessages: IncreaseCounter
}

fun onInternalMessage(in: InMessage) {
    val msg = lazy IncreaseCounter.fromSlice(in.body);
    match (msg) {
        IncreaseCounter => {}
        else => {}
    }
}

fun onBouncedMessage(_: InMessageBounced) {}
";

const PRECOMPILED_MESSAGE_TYPES: &str = r"
struct (0x7e8764ef) IncreaseCounter {
    increaseBy: uint32
}

contract Precompiled {
    incomingMessages: IncreaseCounter
}
";

const MAINNET_USDT_WALLET_TYPES: &str = r"
struct MainnetUsdtWalletStorage {
    status: uint4
    jettonBalance: coins
    ownerAddress: address
    minterAddress: address
}

struct (0xd372158c) MainnetUsdtTopUp {
    queryId: uint64
}

contract MainnetUsdtWallet {
    storage: MainnetUsdtWalletStorage
    incomingMessages: MainnetUsdtTopUp
}
";

fn compiled_runtime_boc_bytes() -> Vec<u8> {
    let source_project = ProjectBuilder::new("aw-stdlib-build-precompiled-source")
        .contract_with_output("simple", SIMPLE_RUNTIME_CONTRACT, "contracts/simple.boc")
        .build();

    source_project.acton().build().run().success();

    fs::read(source_project.path().join("contracts/simple.boc"))
        .expect("must read compiled boc bytes")
}

fn compiled_precompiled_message_boc_bytes() -> Vec<u8> {
    let source_project = ProjectBuilder::new("aw-stdlib-build-precompiled-message-source")
        .contract_with_output(
            "precompiled",
            PRECOMPILED_MESSAGE_CONTRACT,
            "contracts/precompiled.boc",
        )
        .build();

    source_project.acton().build().run().success();

    fs::read(source_project.path().join("contracts/precompiled.boc"))
        .expect("must read compiled precompiled message boc bytes")
}

fn mainnet_usdt_wallet_boc_bytes() -> Vec<u8> {
    fs::read("tests/integration/testdata/usdt/mainnet-wallet.code.boc")
        .expect("must read mainnet USDT wallet BoC fixture")
}

fn mainnet_usdt_wallet_library_boc_bytes() -> Vec<u8> {
    fs::read("tests/integration/testdata/usdt/mainnet-wallet-library.code.boc")
        .expect("must read mainnet USDT wallet library BoC fixture")
}

fn point_precompiled_contract_to_uppercase_boc(project: &crate::support::project::Project) {
    fs::rename(
        project.path().join("contracts/precompiled.boc"),
        project.path().join("contracts/precompiled.BOC"),
    )
    .expect("should rename BoC fixture");

    let manifest_path = project.path().join("Acton.toml");
    let manifest = fs::read_to_string(&manifest_path).expect("should read Acton.toml");
    fs::write(
        &manifest_path,
        manifest.replace("contracts/precompiled.boc", "contracts/precompiled.BOC"),
    )
    .expect("should update Acton.toml");
}

#[test]
fn mainnet_usdt_wallet_library_ref_contract_uses_manifest_metadata_in_transaction_tree() {
    let project = ProjectBuilder::new("aw-stdlib-build-mainnet-usdt-wallet-library-ref")
        .without_acton_toml()
        .contract_from_boc_with_types(
            "MainnetUsdtWallet",
            mainnet_usdt_wallet_boc_bytes(),
            "contracts/MainnetUsdtWallet.types.tolk",
        )
        .contract_from_boc(
            "MainnetUsdtWalletLibrary",
            mainnet_usdt_wallet_library_boc_bytes(),
        )
        .raw_file(
            "contracts/MainnetUsdtWallet.types.tolk",
            MAINNET_USDT_WALLET_TYPES,
        )
        .raw_file(
            "Acton.toml",
            r#"[package]
name = "aw-stdlib-build-mainnet-usdt-wallet-library-ref"
description = "A test project"
version = "0.1.0"

[contracts.MainnetUsdtWallet]
display-name = "Mainnet USDT Wallet Code"
src = "contracts/MainnetUsdtWallet.boc"
types = "contracts/MainnetUsdtWallet.types.tolk"
"#,
        )
        .test_file(
            "build_mainnet_usdt_wallet_library_ref_transaction_tree",
            r#"
            import "../../lib/build"
            import "../../lib/emulation/network"
            import "../../lib/emulation/testing"
            import "../../lib/io"
            import "../contracts/MainnetUsdtWallet.types"

            get fun `test mainnet usdt wallet library ref transaction tree`() {
                testing.registerLibrary(build(
                    "MainnetUsdtWalletLibrary",
                    "contracts/MainnetUsdtWalletLibrary.boc",
                ));

                val deployer = testing.treasury("deployer");
                val init = ContractState {
                    code: build("MainnetUsdtWallet"),
                    data: MainnetUsdtWalletStorage {
                        status: 0,
                        jettonBalance: 0,
                        ownerAddress: deployer.address,
                        minterAddress: deployer.address,
                    }.toCell(),
                };

                val txs = net.send(deployer.address, createMessage({
                    bounce: false,
                    value: ton("0.1"),
                    dest: {
                        stateInit: init,
                    },
                    body: MainnetUsdtTopUp {
                        queryId: 0,
                    },
                }));

                println(txs);
            }
        "#,
        )
        .build();

    project
        .acton()
        .test()
        .show_bodies()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/build_reads_explicit_boc_path_and_executes_runtime_code/mainnet_usdt_wallet_library_ref_contract_uses_manifest_metadata_in_transaction_tree.stdout.txt",
        );
}

#[test]
fn test_precompiled_boc_with_types_prints_decoded_transaction_tree() {
    let boc_bytes = compiled_precompiled_message_boc_bytes();
    let project = ProjectBuilder::new("aw-stdlib-build-precompiled-boc-tree")
        .contract_from_boc_with_types("precompiled", boc_bytes, "contracts/precompiled.types.tolk")
        .raw_file(
            "contracts/precompiled.types.tolk",
            PRECOMPILED_MESSAGE_TYPES,
        )
        .test_file(
            "build_precompiled_boc_transaction_tree",
            r#"
            import "../../lib/build"
            import "../../lib/emulation/network"
            import "../../lib/emulation/testing"
            import "../../lib/io"
            import "../contracts/precompiled.types"

            get fun `test precompiled boc transaction tree`() {
                val sender = testing.treasury("deployer");
                val init = ContractState {
                    code: build("precompiled"),
                    data: createEmptyCell(),
                };
                val address = AutoDeployAddress { stateInit: init }.calculateAddress();

                net.send(sender.address, createMessage({
                    bounce: false,
                    value: ton("1"),
                    dest: {
                        stateInit: init,
                    },
                }));

                val txs = net.send(sender.address, createMessage({
                    bounce: false,
                    value: ton("0.1"),
                    dest: address,
                    body: IncreaseCounter {
                        increaseBy: 5,
                    },
                }));

                println(txs);
            }
        "#,
        )
        .build();

    project
        .acton()
        .test()
        .show_bodies()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/build_reads_explicit_boc_path_and_executes_runtime_code/test_precompiled_boc_with_types_prints_decoded_transaction_tree.stdout.txt",
        );
}

#[test]
fn build_reads_explicit_boc_path_and_executes_runtime_code() {
    let project = ProjectBuilder::new("aw-stdlib-build-boc-path-runtime")
        .contract_with_output("simple", SIMPLE_RUNTIME_CONTRACT, "contracts/simple.boc")
        .test_file(
            "build_boc_path_runtime",
            r#"
            import "../../lib/build"
            import "../../lib/emulation/network"
            import "../../lib/emulation/testing"
            import "../../lib/testing/expect"

            get fun `test aw build boc path runtime`() {
                val fromSource = build("simple");
                val fromBocPath = build("simple", "contracts/simple.boc");
                expect(fromBocPath).toEqual(fromSource);

                val sender = testing.treasury("deployer");
                val init = ContractState {
                    code: fromBocPath,
                    data: createEmptyCell(),
                };
                val address = AutoDeployAddress { stateInit: init }.calculateAddress();

                val deployMsg = createMessage({
                    bounce: false,
                    value: ton("1"),
                    dest: {
                        stateInit: init,
                    },
                });
                expect(net.send(sender.address, deployMsg)).toHaveSuccessfulDeploy({ to: address });
                expect(net.runGetMethod<int>(address, "ping")).toEqual(7);
            }
        "#,
        )
        .build();

    project.acton().build().run().success();

    project
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/build_reads_explicit_boc_path_and_executes_runtime_code/build_reads_explicit_boc_path_and_executes_runtime_code.stdout.txt",
        );
}

#[test]
fn build_reads_name_based_precompiled_boc_contract_and_executes_runtime_code() {
    let boc_bytes = compiled_runtime_boc_bytes();
    let project = ProjectBuilder::new("aw-stdlib-build-precompiled-boc-runtime")
        .contract_from_boc("precompiled", boc_bytes)
        .test_file(
            "build_precompiled_boc_runtime",
            r#"
            import "../../lib/build"
            import "../../lib/emulation/network"
            import "../../lib/emulation/testing"
            import "../../lib/testing/expect"

            get fun `test aw build precompiled boc runtime`() {
                val fromName = build("precompiled");
                val fromBocPath = build("precompiled", "contracts/precompiled.boc");
                expect(fromBocPath).toEqual(fromName);

                val sender = testing.treasury("deployer");
                val init = ContractState {
                    code: fromName,
                    data: createEmptyCell(),
                };
                val address = AutoDeployAddress { stateInit: init }.calculateAddress();

                val deployMsg = createMessage({
                    bounce: false,
                    value: ton("1"),
                    dest: {
                        stateInit: init,
                    },
                });
                expect(net.send(sender.address, deployMsg)).toHaveSuccessfulDeploy({ to: address });
                expect(net.runGetMethod<int>(address, "ping")).toEqual(7);
            }
        "#,
        )
        .build();

    project
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/build_reads_explicit_boc_path_and_executes_runtime_code/build_reads_name_based_precompiled_boc_contract_and_executes_runtime_code.stdout.txt",
        );
}

#[test]
fn build_reads_uppercase_precompiled_boc_contract_and_executes_runtime_code() {
    let boc_bytes = compiled_runtime_boc_bytes();
    let project = ProjectBuilder::new("aw-stdlib-build-uppercase-precompiled-boc-runtime")
        .contract_from_boc("precompiled", boc_bytes)
        .test_file(
            "build_uppercase_precompiled_boc_runtime",
            r#"
            import "../../lib/build"
            import "../../lib/emulation/network"
            import "../../lib/emulation/testing"
            import "../../lib/testing/expect"

            get fun `test aw build uppercase precompiled boc runtime`() {
                val fromName = build("precompiled");
                val fromBocPath = build("precompiled", "contracts/precompiled.BOC");
                expect(fromBocPath).toEqual(fromName);

                val sender = testing.treasury("deployer");
                val init = ContractState {
                    code: fromName,
                    data: createEmptyCell(),
                };
                val address = AutoDeployAddress { stateInit: init }.calculateAddress();

                val deployMsg = createMessage({
                    bounce: false,
                    value: ton("1"),
                    dest: {
                        stateInit: init,
                    },
                });
                expect(net.send(sender.address, deployMsg)).toHaveSuccessfulDeploy({ to: address });
                expect(net.runGetMethod<int>(address, "ping")).toEqual(7);
            }
        "#,
        )
        .build();
    point_precompiled_contract_to_uppercase_boc(&project);

    project
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/build_reads_explicit_boc_path_and_executes_runtime_code/build_reads_uppercase_precompiled_boc_contract_and_executes_runtime_code.stdout.txt",
        );
}

#[test]
fn build_reports_missing_explicit_boc_path() {
    ProjectBuilder::new("ax-stdlib-build-missing-boc-path")
        .contract("dummy", SIMPLE_RUNTIME_CONTRACT)
        .test_file(
            "build_missing_boc_path",
            r#"
            import "../../lib/build"

            get fun `test ax build missing boc path`() {
                val _ = build("missing_boc", "contracts/missing.boc");
            }
        "#,
        )
        .build()
        .acton()
        .test()
        .run()
        .failure()
        .assert_failed(1)
        .assert_contains("Cannot read BoC file")
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/build_reads_explicit_boc_path_and_executes_runtime_code/build_reports_missing_explicit_boc_path.stdout.txt",
        );
}

#[test]
fn build_reports_invalid_explicit_boc_path() {
    ProjectBuilder::new("ax-stdlib-build-invalid-boc-path")
        .contract("dummy", SIMPLE_RUNTIME_CONTRACT)
        .raw_file("contracts/invalid.boc", "not a boc payload")
        .test_file(
            "build_invalid_boc_path",
            r#"
            import "../../lib/build"

            get fun `test ax build invalid boc path`() {
                val _ = build("invalid_boc", "contracts/invalid.boc");
            }
        "#,
        )
        .build()
        .acton()
        .test()
        .run()
        .failure()
        .assert_failed(1)
        .assert_contains("Failed to decode code BoC")
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/build_reads_explicit_boc_path_and_executes_runtime_code/build_reports_invalid_explicit_boc_path.stdout.txt",
        );
}

#[test]
fn build_name_based_code_executes_in_fixture_runtime() {
    FixtureProject::load("basic")
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(2)
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/build_reads_explicit_boc_path_and_executes_runtime_code/build_name_based_code_executes_in_fixture_runtime.stdout.txt",
        );
}
