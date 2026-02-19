use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;
use std::fs;
use std::path::Path;

fn write_acton_toml(project_root: &Path, toml_content: &str) {
    fs::write(project_root.join("Acton.toml"), toml_content).expect("write Acton.toml");
}

fn append_build_output_fift(project_root: &Path, output_fift: &str) {
    let acton_toml_path = project_root.join("Acton.toml");
    let mut acton_toml = fs::read_to_string(&acton_toml_path).expect("read Acton.toml");
    acton_toml.push_str(&format!("\n[build]\noutput-fift = \"{output_fift}\"\n"));
    fs::write(acton_toml_path, acton_toml).expect("write Acton.toml with [build] section");
}

#[test]
fn build_supports_quoted_contract_keys_in_dependency_resolution() {
    let project = ProjectBuilder::new("build-config-edge-quoted-keys")
        .raw_file(
            "contracts/child.lib.tolk",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
        )
        .raw_file(
            "contracts/parent-contract.tolk",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
        )
        .build();

    write_acton_toml(
        project.path(),
        r#"[package]
name = "build-config-edge-quoted-keys"
description = ""
version = "0.1.0"

[contracts."child.lib"]
name = "Child Library"
src = "contracts/child.lib.tolk"
depends = []

[contracts.parent-contract]
name = "Parent Contract"
src = "contracts/parent-contract.tolk"
depends = ["child.lib"]
"#,
    );

    project
        .acton()
        .build()
        .run()
        .success()
        .assert_contains("Compiling Child Library")
        .assert_contains("Compiling Parent Contract");

    assert!(
        project.path().join("build/child.lib.json").exists(),
        "build artifact should use quoted contract key for child contract"
    );
    assert!(
        project.path().join("build/parent-contract.json").exists(),
        "build artifact should use hyphenated contract key for parent contract"
    );

    let generated_dep = fs::read_to_string(project.path().join("gen/child.lib_code.tolk"))
        .expect("read generated dependency file");
    assert!(
        generated_dep.contains("fun child_libCompiledCode(): cell"),
        "generated dependency function should normalize dotted key into valid identifier"
    );
}

#[test]
fn build_ignores_empty_output_fift_path_in_project_config() {
    let project = ProjectBuilder::new("build-config-edge-empty-output-fift")
        .contract(
            "simple",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
        )
        .build();

    append_build_output_fift(project.path(), "");

    project
        .acton()
        .build()
        .run()
        .success()
        .assert_contains("Finished");

    assert!(
        !project.path().join("build/fift/simple.fif").exists(),
        "empty [build].output-fift should disable fift output emission"
    );
}

#[test]
fn build_cli_output_fift_overrides_empty_output_fift_config_value() {
    let project = ProjectBuilder::new("build-config-edge-cli-overrides-empty-output-fift")
        .contract(
            "simple",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
        )
        .build();

    append_build_output_fift(project.path(), "");

    project
        .acton()
        .build()
        .with_output_fift("cli/fift")
        .run()
        .success()
        .assert_contains("Finished");

    assert!(
        project.path().join("cli/fift/simple.fif").exists(),
        "CLI --output-fift should take precedence over empty config output-fift"
    );
}

#[test]
fn build_reports_parse_error_when_dependency_object_omits_name() {
    let project = ProjectBuilder::new("build-config-edge-invalid-dependency-object")
        .raw_file(
            "contracts/child.tolk",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
        )
        .raw_file(
            "contracts/root.tolk",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
        )
        .build();

    write_acton_toml(
        project.path(),
        r#"[package]
name = "build-config-edge-invalid-dependency-object"
description = ""
version = "0.1.0"

[contracts.child]
name = "Child"
src = "contracts/child.tolk"
depends = []

[contracts.root]
name = "Root"
src = "contracts/root.tolk"
depends = [{ kind = "library_ref", function = "childCode" }]
"#,
    );

    project
        .acton()
        .build()
        .run()
        .failure()
        .assert_stderr_contains("TOML parse error")
        .assert_stderr_contains("ContractDependency")
        .assert_stderr_contains("did not match any variant");
}

#[test]
fn build_reports_parse_error_for_non_string_non_object_dependency_entry() {
    let project = ProjectBuilder::new("build-config-edge-invalid-dependency-entry-type")
        .raw_file(
            "contracts/child.tolk",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
        )
        .raw_file(
            "contracts/root.tolk",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
        )
        .build();

    write_acton_toml(
        project.path(),
        r#"[package]
name = "build-config-edge-invalid-dependency-entry-type"
description = ""
version = "0.1.0"

[contracts.child]
name = "Child"
src = "contracts/child.tolk"
depends = []

[contracts.root]
name = "Root"
src = "contracts/root.tolk"
depends = [42]
"#,
    );

    project
        .acton()
        .build()
        .run()
        .failure()
        .assert_stderr_contains("TOML parse error")
        .assert_stderr_contains("ContractDependency")
        .assert_stderr_contains("did not match any variant");
}

#[test]
fn build_reports_parse_error_when_depends_is_not_an_array() {
    let project = ProjectBuilder::new("build-config-edge-invalid-depends-shape")
        .raw_file(
            "contracts/child.tolk",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
        )
        .raw_file(
            "contracts/root.tolk",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
        )
        .build();

    write_acton_toml(
        project.path(),
        r#"[package]
name = "build-config-edge-invalid-depends-shape"
description = ""
version = "0.1.0"

[contracts.child]
name = "Child"
src = "contracts/child.tolk"
depends = []

[contracts.root]
name = "Root"
src = "contracts/root.tolk"
depends = { name = "child" }
"#,
    );

    project
        .acton()
        .build()
        .run()
        .failure()
        .assert_stderr_contains("TOML parse error")
        .assert_stderr_contains("expected a sequence");
}

#[test]
fn build_allows_output_fift_path_with_spaces_in_project_config() {
    let project = ProjectBuilder::new("build-config-edge-output-fift-path-with-spaces")
        .contract(
            "simple",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
        )
        .build();

    append_build_output_fift(project.path(), "custom fift/out files");

    project
        .acton()
        .build()
        .run()
        .success()
        .assert_contains("Finished");

    assert!(
        project
            .path()
            .join("custom fift/out files/simple.fif")
            .exists(),
        "output-fift path with spaces from [build] should produce a .fif file"
    );
}

#[test]
fn build_handles_mixed_depends_presence_across_contract_entries() {
    let project = ProjectBuilder::new("build-config-edge-mixed-depends-presence")
        .raw_file(
            "contracts/base.tolk",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
        )
        .raw_file(
            "contracts/root.tolk",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
        )
        .build();

    write_acton_toml(
        project.path(),
        r#"[package]
name = "build-config-edge-mixed-depends-presence"
description = ""
version = "0.1.0"

[contracts.base]
name = "Base"
src = "contracts/base.tolk"

[contracts.root]
name = "Root"
src = "contracts/root.tolk"
depends = ["base"]
"#,
    );

    project
        .acton()
        .build()
        .run()
        .success()
        .assert_contains("Compiling Base")
        .assert_contains("Compiling Root");

    assert!(
        project.path().join("build/base.json").exists(),
        "base contract should build even when its own depends field is omitted"
    );
    assert!(
        project.path().join("build/root.json").exists(),
        "root contract should build when depending on base"
    );
    assert!(
        project.path().join("gen/base_code.tolk").exists(),
        "dependency code should still be generated for root -> base dependency"
    );
}

#[test]
fn build_reports_parse_error_for_dependency_object_with_unknown_kind() {
    let project = ProjectBuilder::new("build-config-edge-invalid-dependency-kind")
        .raw_file(
            "contracts/child.tolk",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
        )
        .raw_file(
            "contracts/root.tolk",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
        )
        .build();

    write_acton_toml(
        project.path(),
        r#"[package]
name = "build-config-edge-invalid-dependency-kind"
description = ""
version = "0.1.0"

[contracts.child]
name = "Child"
src = "contracts/child.tolk"
depends = []

[contracts.root]
name = "Root"
src = "contracts/root.tolk"
depends = [{ name = "child", kind = "dynamic_ref" }]
"#,
    );

    project
        .acton()
        .build()
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/build/build_cmd_project_config_edge_tests/build_reports_parse_error_for_dependency_object_with_unknown_kind.stderr.txt",
        );
}

#[test]
fn build_reports_semantic_error_for_empty_simple_dependency_name() {
    let project = ProjectBuilder::new("build-config-edge-empty-simple-dependency-name")
        .raw_file(
            "contracts/child.tolk",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
        )
        .raw_file(
            "contracts/root.tolk",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
        )
        .build();

    write_acton_toml(
        project.path(),
        r#"[package]
name = "build-config-edge-empty-simple-dependency-name"
description = ""
version = "0.1.0"

[contracts.child]
name = "Child"
src = "contracts/child.tolk"
depends = []

[contracts.root]
name = "Root"
src = "contracts/root.tolk"
depends = [""]
"#,
    );

    project
        .acton()
        .build()
        .run()
        .failure()
        .assert_stderr_contains("depends on ''")
        .assert_stderr_contains("not defined in Acton.toml");
}

#[test]
fn build_reports_semantic_error_for_empty_dependency_name_in_detailed_object() {
    let project = ProjectBuilder::new("build-config-edge-empty-detailed-dependency-name")
        .raw_file(
            "contracts/child.tolk",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
        )
        .raw_file(
            "contracts/root.tolk",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
        )
        .build();

    write_acton_toml(
        project.path(),
        r#"[package]
name = "build-config-edge-empty-detailed-dependency-name"
description = ""
version = "0.1.0"

[contracts.child]
name = "Child"
src = "contracts/child.tolk"
depends = []

[contracts.root]
name = "Root"
src = "contracts/root.tolk"
depends = [{ name = "", kind = "embed_code", function = "childCode" }]
"#,
    );

    project
        .acton()
        .build()
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/build/build_cmd_project_config_edge_tests/build_reports_semantic_error_for_empty_dependency_name_in_detailed_object.stderr.txt",
        );
}

#[test]
fn build_defaults_detailed_dependency_kind_to_embed_code_when_kind_is_omitted() {
    let project = ProjectBuilder::new("build-config-edge-default-detailed-dependency-kind")
        .raw_file(
            "contracts/child.tolk",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
        )
        .raw_file(
            "contracts/root.tolk",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
        )
        .build();

    write_acton_toml(
        project.path(),
        r#"[package]
name = "build-config-edge-default-detailed-dependency-kind"
description = ""
version = "0.1.0"

[contracts.child]
name = "Child"
src = "contracts/child.tolk"
depends = []

[contracts.root]
name = "Root"
src = "contracts/root.tolk"
depends = [{ name = "child" }]
"#,
    );

    project
        .acton()
        .build()
        .run()
        .success()
        .assert_contains("Compiling Child")
        .assert_contains("Compiling Root");

    let generated_dep = fs::read_to_string(project.path().join("gen/child_code.tolk"))
        .expect("read generated dependency file");
    assert!(
        generated_dep.contains("fun childCompiledCode(): cell asm"),
        "detailed dependency object without kind should keep default function naming"
    );
    assert!(
        generated_dep.contains("base64>B B>boc PUSHREF"),
        "detailed dependency object without kind should default to embed_code asm"
    );
    assert!(
        !generated_dep.contains("hashu"),
        "detailed dependency object without kind should not use library_ref asm"
    );
}
