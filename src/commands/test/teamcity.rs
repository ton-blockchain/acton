use crate::commands::test;
use crate::context::{AssertFailure, BuildCache};
use abi::ContractAbi;
use std::collections::HashMap;

pub struct TeamcityReporter;

impl TeamcityReporter {
    fn escape_name(name: &str) -> String {
        // See https://www.jetbrains.com/help/teamcity/service-messages.html#Escaped+Values
        name.replace("|", "||")
            .replace("\n", "|n")
            .replace("\r", "|r")
            .replace("[", "|[")
            .replace("]", "|]")
            .replace("'", "|'")
    }

    pub fn on_testing_started() {
        println!("##teamcity[testingStarted]");
    }
    pub fn on_testing_finished() {
        println!("##teamcity[testingFinished]");
    }

    pub fn on_test_suite_started(file_path: &str) {
        let file_name = std::path::Path::new(file_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(file_path);
        let name = Self::escape_name(file_name);
        println!(
            "##teamcity[testSuiteStarted name='{}' nodeId='suite_{}' parentNodeId='0' nodeType='file'  locationHint='file://{}']",
            name, name, file_path
        );
    }

    pub fn on_test_suite_finished(file_path: &str) {
        let file_name = std::path::Path::new(file_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(file_path);
        let name = Self::escape_name(file_name);
        println!(
            "##teamcity[testSuiteFinished name='{}' nodeId='suite_{}' ]",
            name, name
        );
    }

    pub fn on_test_started(test_name: &str, file_path: &str) {
        let name = Self::escape_name(test_name);
        let file_name = std::path::Path::new(file_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(file_path);
        let suite_name = Self::escape_name(file_name);
        let location = format!("{}:{}", file_path, test_name);
        println!(
            "##teamcity[testStarted name='{}' nodeId='test_{}' parentNodeId='suite_{}' locationHint='tolk_qn://{}' ]",
            name, name, suite_name, location
        );
    }

    pub fn on_test_finished(test_name: &str, file_path: &str, duration_ms: u128) {
        let name = Self::escape_name(test_name);
        let file_name = std::path::Path::new(file_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(file_path);
        let suite_name = Self::escape_name(file_name);
        println!(
            "##teamcity[testFinished name='{}' nodeId='test_{}' duration='{}' parentNodeId='suite_{}' ]",
            name, name, duration_ms, suite_name
        );
    }

    pub fn on_test_failed(
        test_name: &str,
        duration_ms: u128,
        assert_failure: Option<&AssertFailure>,
        abi: &&ContractAbi,
    ) {
        let name = Self::escape_name(test_name);

        if let Some(assert_failure) = assert_failure {
            let details = assert_failure.location().unwrap_or("".to_string());
            if let AssertFailure::Bin(bin_failure) = assert_failure {
                if bin_failure.operator == "==" {
                    let expected = test::format_tuple_value(
                        &bin_failure.right,
                        &bin_failure.right_type,
                        &HashMap::new(), // empty accounts for simple formatting
                        &abi,
                        &BuildCache::new(),
                        0,
                    );
                    let actual = test::format_tuple_value(
                        &bin_failure.left,
                        &bin_failure.left_type,
                        &HashMap::new(),
                        &abi,
                        &BuildCache::new(),
                        0,
                    );

                    println!(
                        "##teamcity[testFailed type='comparisonFailure' name='{}' nodeId='test_{}' duration='{}' message='Values are not equal' details='{}' expected='{}' actual='{}']",
                        name,
                        name,
                        duration_ms,
                        Self::escape_name(&details),
                        Self::escape_name(&expected),
                        Self::escape_name(&actual),
                    );
                    return;
                } else if bin_failure.operator == "!=" {
                    println!(
                        "##teamcity[testFailed name='{}' nodeId='test_{}' duration='{}' message='Values are equal but expected to be different' details='{}']",
                        name,
                        name,
                        Self::escape_name(&details),
                        duration_ms
                    );
                    return;
                }
            }

            println!(
                "##teamcity[testFailed name='{}' nodeId='test_{}' duration='{}' message='Assertion failed' details='{}']",
                name,
                name,
                duration_ms,
                Self::escape_name(&details),
            );
        }

        println!(
            "##teamcity[testFailed name='{}' nodeId='test_{}' duration='{}' message='Test failed']",
            name, name, duration_ms
        );
    }

    pub fn on_test_ignored(test_name: &str, duration_ms: u128) {
        let name = Self::escape_name(test_name);
        println!(
            "##teamcity[testIgnored name='{}' nodeId='test_{}' duration='{}' ]",
            name, name, duration_ms
        );
    }
}
