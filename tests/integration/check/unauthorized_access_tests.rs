use crate::integration::check::run_simple_test;
use function_name::named;

#[test]
#[named]
fn test_check_storage_write_without_admin_check_detects_storage_save() {
    run_simple_test(
        "unauthorized_access",
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
    run_simple_test(
        "unauthorized_access",
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
    run_simple_test(
        "unauthorized_access",
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
