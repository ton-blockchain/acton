use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

mod acton_import_tests;
mod asm_safety_comment_tests;
mod bless_safety_comment_tests;
mod create_message_body_to_cell_tests;
mod dangerous_send_mode_safety_comment_tests;
mod deprecated_tests;
mod dict_type_use_tests;
mod divide_before_multiply_tests;
mod duplicated_condition_tests;
mod enable_only_tests;
mod enum_cast_safety_comment_tests;
mod explicit_return_type_tests;
mod field_init_can_be_folded_tests;
mod identical_conditional_branches_tests;
mod import_path_can_use_mappings_tests;
mod lint_exclude_tests;
mod lint_exit_code_tests;
mod lint_output_github_tests;
mod lint_output_gitlab_tests;
mod lint_output_json_tests;
mod lint_output_plain_tests;
mod lint_output_sarif_tests;
mod lint_rules_config_tests;
mod message_entity_naming_tests;
mod method_can_be_static_tests;
mod missing_contract_header_tests;
mod mutable_parameter_can_be_immutable_tests;
mod mutable_variable_can_be_immutable_tests;
mod name_case_checker_tests;
mod negated_is_type_can_use_not_is_tests;
mod no_bounce_handler_tests;
mod no_global_variables_tests;
mod pure_function_call_unused_tests;
mod random_requires_initialization_tests;
mod reserve_mode_literal_tests;
mod script_roots_tests;
mod send_mode_literal_tests;
mod several_not_null_assertions_tests;
mod throw_requires_documented_error_value_tests;
mod throw_requires_errors_enum_tests;
mod type_inference_regressions_tests;
mod unauthorized_access_tests;
mod unused_expression_tests;
mod unused_import_tests;
mod unused_variable_tests;
mod used_ignored_identifier_tests;
mod write_only_variable_tests;

pub(crate) fn run_rule_test(group: &str, rule_code: &str, content: &str, name: &str) {
    run_rule_test_with_mappings(group, rule_code, content, &[], &[], name);
}

pub(crate) fn run_rule_test_with_mappings(
    group: &str,
    rule_code: &str,
    content: &str,
    files: &[(&str, &str)],
    mappings: &[(&str, &str)],
    name: &str,
) {
    let mut builder = ProjectBuilder::new(&format!("check-{name}")).contract("main", content);
    for (path, file_content) in files {
        builder = builder.file(path, file_content);
    }
    for (mapping_name, mapping_target) in mappings {
        builder = builder.mapping(mapping_name, mapping_target);
    }

    let project = builder.build();

    project.acton().init().run().success();

    project
        .acton()
        .check()
        .arg("--enable-only")
        .arg(rule_code)
        .run()
        .success()
        .assert_stderr_snapshot_matches(&format!("integration/snapshots/check/{group}/{name}.txt"));
}

pub(crate) fn run_rule_fix_test(rule_code: &str, before: &str, after: &str, name: &str) {
    run_rule_fix_test_with_mappings(rule_code, before, after, &[], &[], name);
}

pub(crate) fn run_rule_fix_test_with_files(
    rule_code: &str,
    before: &str,
    after: &str,
    files: &[(&str, &str)],
    name: &str,
) {
    run_rule_fix_test_with_mappings(rule_code, before, after, files, &[], name);
}

pub(crate) fn run_rule_fix_test_with_mappings(
    rule_code: &str,
    before: &str,
    after: &str,
    files: &[(&str, &str)],
    mappings: &[(&str, &str)],
    name: &str,
) {
    let mut builder = ProjectBuilder::new(&format!("check-fix-{name}")).contract("main", before);
    for (path, content) in files {
        builder = builder.file(path, content);
    }
    for (name, target) in mappings {
        builder = builder.mapping(name, target);
    }

    let project = builder.build();

    project.acton().init().run().success();
    project
        .acton()
        .check()
        .arg("--enable-only")
        .arg(rule_code)
        .arg("--fix")
        .run()
        .success();

    let file_path = project.path().join("contracts/main.tolk");
    let actual = std::fs::read_to_string(&file_path)
        .unwrap_or_else(|e| panic!("failed to read fixed file '{}': {}", file_path.display(), e));

    assert_eq!(
        actual.trim(),
        after.trim(),
        "fixed file content mismatch for {}",
        file_path.display()
    );
}

pub(crate) fn run_rule_check_test_with_files(
    group: &str,
    rule_code: &str,
    main_content: &str,
    files: &[(&str, &str)],
    name: &str,
) {
    run_rule_check_test_with_files_and_mappings(group, rule_code, main_content, files, &[], name);
}

pub(crate) fn run_rule_check_test_with_files_and_mappings(
    group: &str,
    rule_code: &str,
    main_content: &str,
    files: &[(&str, &str)],
    mappings: &[(&str, &str)],
    name: &str,
) {
    let mut builder = ProjectBuilder::new(&format!("check-{name}")).contract("main", main_content);
    for (path, content) in files {
        builder = builder.file(path, content);
    }
    for (name, target) in mappings {
        builder = builder.mapping(name, target);
    }

    let project = builder.build();

    project.acton().init().run().success();
    project
        .acton()
        .check()
        .arg("--enable-only")
        .arg(rule_code)
        .run()
        .success()
        .assert_stderr_snapshot_matches(&format!("integration/snapshots/check/{group}/{name}.txt"));
}
