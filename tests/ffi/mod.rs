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

    #[test]
    fn test_get_config() {
        ProjectBuilder::new("simple")
            .contract("simple", SIMPLE_CONTRACT)
            .test_file_from_path("test", "tests/ffi/config.test.tolk")
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
            .test_file_from_path("test", "tests/ffi/config.test.tolk")
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
            .test_file_from_path("test", "tests/ffi/config.test.tolk")
            .build()
            .acton()
            .test()
            .filter("test bad config")
            .run()
            .success()
            .assert_passed(1);
    }

    #[test]
    fn test_get_executor_config() {
        ProjectBuilder::new("simple")
            .contract("simple", SIMPLE_CONTRACT)
            .test_file_from_path("test", "tests/ffi/config.test.tolk")
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
            .test_file_from_path("test", "tests/ffi/config.test.tolk")
            .build()
            .acton()
            .test()
            .filter("test current config")
            .run()
            .success()
            .assert_passed(1);
    }
}
