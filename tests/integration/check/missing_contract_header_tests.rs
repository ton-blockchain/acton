use crate::integration::check::run_rule_test;
use function_name::named;

const RULE_CODE: &str = "E031";

#[test]
#[named]
fn test_check_missing_contract_header_reports_contract_entrypoint_without_header() {
    run_rule_test(
        "missing_contract_header",
        RULE_CODE,
        r"
            struct Storage {
                owner: address
            }

            fun onInternalMessage(in: InMessage) {
                debug.print(in.senderAddress);
            }
        ",
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_missing_contract_header_skips_file_with_contract_header() {
    run_rule_test(
        "missing_contract_header",
        RULE_CODE,
        r"
            struct Storage {
                owner: address
            }

            contract Wallet {
                storage: Storage
            }

            fun onInternalMessage(in: InMessage) {
                debug.print(in.senderAddress);
            }
        ",
        function_name!(),
    );
}
