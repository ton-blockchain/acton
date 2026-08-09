use std::fmt::Write as _;

use expect_test::{Expect, expect};
use ton_language_server_core::languages::tolk::{LANGUAGE_ID, TolkLanguage};
use ton_language_server_core::{
    Diagnostic, DocumentUri, LanguageService, LanguageServiceConfig, WorkspaceConfig,
};

const URI: &str = "file:///workspace/main.tolk";

fn check(source: &str, manifest: &str, expect: Expect) {
    let uri = DocumentUri::from(URI);
    let mut service = LanguageService::new(LanguageServiceConfig::default());
    service.register_language(TolkLanguage::new());
    service
        .set_workspace_config(
            LANGUAGE_ID,
            WorkspaceConfig::new("file:///workspace", None, manifest),
        )
        .expect("workspace configuration should be accepted");
    service
        .open_document(uri.clone(), LANGUAGE_ID, 1, source)
        .expect("Tolk document should open");
    let diagnostics = service
        .diagnostics(&uri)
        .expect("diagnostics request should succeed");

    expect.assert_eq(&render(&diagnostics));
}

fn render(diagnostics: &[Diagnostic]) -> String {
    if diagnostics.is_empty() {
        return "<none>".to_owned();
    }

    let mut output = String::new();
    for diagnostic in diagnostics {
        if !output.is_empty() {
            output.push('\n');
        }
        let _ = write!(
            &mut output,
            "{}:{}-{}:{} {:?} {} {}",
            diagnostic.range.start.line,
            diagnostic.range.start.character,
            diagnostic.range.end.line,
            diagnostic.range.end.character,
            diagnostic.severity,
            diagnostic.code.as_deref().unwrap_or("<no-code>"),
            diagnostic.message.replace('\n', " | "),
        );
        if !diagnostic.tags.is_empty() {
            let _ = write!(&mut output, " {:?}", diagnostic.tags);
        }
    }
    output
}

#[test]
fn reports_linter_diagnostics_with_rule_codes_and_ranges() {
    check(
        "fun BadName() {}",
        "",
        expect![[r"
            0:4-0:11 Warning S001 name should be in the expected case | not camelCase: `BadName`"]],
    );
}

#[test]
fn honors_global_allow_and_deny_levels() {
    check(
        "fun BadName() {}",
        r#"
            [lint.rules]
            name-case-checker = "allow"
        "#,
        expect!["<none>"],
    );
    check(
        "fun BadName() {}",
        r#"
            [lint.rules]
            name-case-checker = "deny"
        "#,
        expect![[r"
            0:4-0:11 Error S001 name should be in the expected case | not camelCase: `BadName`"]],
    );
}

#[test]
fn honors_contract_specific_rule_levels() {
    check(
        "fun BadName() {}",
        r#"
            [contracts.main]
            src = "main.tolk"

            [lint.rules]
            name-case-checker = "allow"

            [lint.rules.main]
            name-case-checker = "deny"
        "#,
        expect![[r"
            0:4-0:11 Error S001 name should be in the expected case | not camelCase: `BadName`"]],
    );
}

#[test]
fn honors_inline_suppressions() {
    check(
        "// check-disable-next-line name-case-checker\nfun BadName() {}",
        "",
        expect!["<none>"],
    );
}

#[test]
fn preserves_deprecated_diagnostic_tags() {
    check(
        r#"
            @deprecated("use bar instead")
            fun foo() {}

            fun main() {
                foo();
            }
        "#,
        r#"
            [lint.rules]
            deprecated-symbol-use = "warn"
        "#,
        expect![[r"
            5:16-5:19 Warning E003 usage of deprecated symbol | foo is deprecated and should not be used. use bar instead | deprecated symbols may be removed in future versions [Deprecated]"]],
    );
}

#[test]
fn reports_utf16_ranges_after_non_bmp_characters() {
    check(
        "fun main() { /* 😀 */ val BadLocal = 1; return BadLocal; }",
        "",
        expect![[r"
            0:26-0:34 Warning S001 name should be in the expected case | not camelCase: BadLocal [Unnecessary]"]],
    );
}

#[test]
fn refreshes_diagnostics_after_document_changes() {
    let uri = DocumentUri::from(URI);
    let mut service = LanguageService::new(LanguageServiceConfig::default());
    service.register_language(TolkLanguage::new());
    service
        .open_document(uri.clone(), LANGUAGE_ID, 1, "fun BadName() {}")
        .expect("Tolk document should open");
    let before = service
        .diagnostics(&uri)
        .expect("diagnostics request should succeed");
    service
        .change_document(&uri, 2, "fun goodName() {}")
        .expect("Tolk document should change");
    let after = service
        .diagnostics(&uri)
        .expect("diagnostics request should succeed");

    expect![[r"
        before:
        0:4-0:11 Warning S001 name should be in the expected case | not camelCase: `BadName`
        after:
        <none>"]]
    .assert_eq(&format!(
        "before:\n{}\nafter:\n{}",
        render(&before),
        render(&after)
    ));
}
