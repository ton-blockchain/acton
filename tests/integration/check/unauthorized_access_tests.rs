use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;
use function_name::named;

fn run_unauthorized_access_test(content: &str, name: &str) {
    let project = ProjectBuilder::new(&format!("check-{name}"))
        .contract("main", content)
        .with_lint_level("unauthorized-access", "warn")
        .build();

    project.acton().init().run().success();

    project
        .acton()
        .check()
        .run()
        .success()
        .assert_stderr_snapshot_matches(&format!(
            "integration/snapshots/check/unauthorized_access/{name}.txt"
        ));
}

#[test]
#[named]
fn test_check_storage_write_without_admin_check_detects_storage_save() {
    run_unauthorized_access_test(
        r#"
            struct Storage {
                adminAddress: address
            }

            fun save(_storage: Storage) {
                contract.setData(contract.getData());
            }

            fun onInternalMessage(in: InMessage) {
                val storage = Storage {
                    adminAddress: in.senderAddress,
                };
                save(storage);
            }
        "#,
        function_name!(),
    )
}

#[test]
#[named]
fn test_check_storage_write_without_admin_check_detects_contract_set_data() {
    run_unauthorized_access_test(
        r#"
            fun onInternalMessage(in: InMessage) {
                val _sender = in.senderAddress;
                contract.setData(contract.getData());
            }
        "#,
        function_name!(),
    )
}

#[test]
#[named]
fn test_check_storage_write_without_admin_check_skips_guarded_storage_write() {
    run_unauthorized_access_test(
        r#"
            struct Storage {
                adminAddress: address
            }

            fun save(_storage: Storage) {
                contract.setData(contract.getData());
            }

            fun onInternalMessage(in: InMessage) {
                val storage = Storage {
                    adminAddress: in.senderAddress,
                };
                assert (in.senderAddress == storage.adminAddress) throw 100;
                save(storage);
            }
        "#,
        function_name!(),
    )
}
