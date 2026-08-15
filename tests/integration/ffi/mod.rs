#[cfg(test)]
mod tests {
    use crate::support::TestOutputExt;
    use crate::support::project::ProjectBuilder;

    const SIMPLE_CONTRACT: &str = r#"
fun onInternalMessage(in: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
fun calculateGasFee(workchain: int8, gasUsed: int): coins
    asm(gasUsed workchain) "GETGASFEE"
get fun getParam() {
    return calculateGasFee(0, 100000);
}
"#;

    const CONFIG_READER_CONTRACT: &str = r"
fun onInternalMessage(in: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
get fun readNegParam(): int {
    val c = blockchain.configParam(-137);
    return c!.beginParse().loadUint(32);
}
";

    #[test]
    fn test_get_config() {
        ProjectBuilder::new("simple")
            .contract("simple", SIMPLE_CONTRACT)
            .test_file_from_path("test", "tests/integration/ffi/config.test.tolk")
            .build()
            .acton()
            .test()
            .filter("test get config")
            .run()
            .success()
            .assert_passed(1);
    }

    #[test]
    fn test_set_config() {
        ProjectBuilder::new("simple")
            .contract("simple", SIMPLE_CONTRACT)
            .test_file_from_path("test", "tests/integration/ffi/config.test.tolk")
            .build()
            .acton()
            .test()
            .filter("test set config")
            .run()
            .success()
            .assert_passed(1);
    }

    #[test]
    fn test_bad_config() {
        ProjectBuilder::new("simple")
            .contract("simple", SIMPLE_CONTRACT)
            .test_file_from_path("test", "tests/integration/ffi/config.test.tolk")
            .build()
            .acton()
            .test()
            .filter("test bad config")
            .run()
            .success()
            .assert_passed(1);
    }

    #[test]
    fn test_set_negative_config_param() {
        ProjectBuilder::new("simple")
            .contract("simple", SIMPLE_CONTRACT)
            .test_file_from_path("test", "tests/integration/ffi/config.test.tolk")
            .build()
            .acton()
            .test()
            .filter("test set negative config param")
            .run()
            .success()
            .assert_passed(1);
    }

    #[test]
    fn test_negative_config_param_read_by_contract() {
        ProjectBuilder::new("simple")
            .contract("config_reader", CONFIG_READER_CONTRACT)
            .test_file_from_path("test", "tests/integration/ffi/config.test.tolk")
            .build()
            .acton()
            .test()
            .filter("test negative config param read by contract")
            .run()
            .success()
            .assert_passed(1);
    }

    #[test]
    fn test_get_executor_config() {
        ProjectBuilder::new("simple")
            .contract("simple", SIMPLE_CONTRACT)
            .test_file_from_path("test", "tests/integration/ffi/config.test.tolk")
            .build()
            .acton()
            .test()
            .filter("test get executor config")
            .run()
            .success()
            .assert_passed(1);
    }

    #[test]
    fn test_current_config() {
        ProjectBuilder::new("simple")
            .contract("simple", SIMPLE_CONTRACT)
            .test_file_from_path("test", "tests/integration/ffi/config.test.tolk")
            .build()
            .acton()
            .test()
            .filter("test current config")
            .run()
            .success()
            .assert_passed(1);
    }

    #[test]
    fn test_get_shard_account() {
        ProjectBuilder::new("simple")
            .contract("simple", SIMPLE_CONTRACT)
            .test_file_from_path("test", "tests/integration/ffi/shard_account.test.tolk")
            .build()
            .acton()
            .test()
            .filter("test get shard account")
            .run()
            .success()
            .assert_passed(1);
    }

    #[test]
    fn test_set_and_get_shard_account() {
        ProjectBuilder::new("simple")
            .contract("simple", SIMPLE_CONTRACT)
            .test_file_from_path("test", "tests/integration/ffi/shard_account.test.tolk")
            .build()
            .acton()
            .test()
            .filter("test set and get shard account")
            .run()
            .success()
            .assert_passed(1);
    }

    #[test]
    fn test_reset_shard_account() {
        ProjectBuilder::new("simple")
            .contract("simple", SIMPLE_CONTRACT)
            .test_file_from_path("test", "tests/integration/ffi/shard_account.test.tolk")
            .build()
            .acton()
            .test()
            .filter("test reset shard account")
            .run()
            .success()
            .assert_passed(1);
    }
}
