use super::support::CompletionTest;
use expect_test::expect;

#[test]
fn completes_workspace_stdlib_and_mapping_imports() {
    // A relative file prefix completes only the final path segment.
    CompletionTest::new(r#"import "./ut<caret>""#)
        .file(
            "utils.tolk",
            r#"
                fun utility() {}
            "#,
        )
        .labels(&["utils", "./utils"])
        .check(expect![[r#"
            label  kind  detail  edit       text
            utils  File  .tolk   0:10-0:12  utils"#]]);

    // A mapping path also completes only its final segment.
    CompletionTest::new(r#"import "@lib/he<caret>""#)
        .manifest(
            r#"
                [import-mappings]
                lib = "/workspace/lib"
            "#,
        )
        .file("lib/helpers.tolk", "fun helper() {}")
        .labels(&["helpers", "@lib/helpers"])
        .check(expect![[r#"
            label    kind  detail  edit       text
            helpers  File  .tolk   0:13-0:15  helpers"#]]);
}

#[test]
fn completes_mapping_roots_and_immediate_directory_entries() {
    let manifest = r#"
        [import-mappings]
        my_lib = "./libs/my_lib"
    "#;

    // An import beginning with @ offers stdlib and every configured mapping root.
    CompletionTest::new(r#"import "@<caret>""#)
        .manifest(manifest)
        .labels(&["@my_lib/", "@stdlib/"])
        .check(expect![[r#"
            label     kind    detail  edit     text
            @my_lib/  Folder          0:8-0:9  @my_lib/
            @stdlib/  Folder          0:8-0:9  @stdlib/"#]]);

    // Inside a mapping, only files and folders immediately below that directory are offered.
    CompletionTest::new(r#"import "@my_lib/<caret>""#)
        .manifest(manifest)
        .file("libs/my_lib/utils.tolk", "fun helper() {}")
        .file("libs/my_lib/nested/other.tolk", "fun other() {}")
        .labels(&["utils", "nested/", "@my_lib/utils"])
        .check(expect![[r#"
            label    kind    detail  edit       text
            nested/  Folder          0:16-0:16  nested/
            utils    File    .tolk   0:16-0:16  utils"#]]);
}

#[test]
fn applies_import_path_completion_without_replacing_parent_segments() {
    // Applying a mapped file completion preserves the typed mapping prefix.
    CompletionTest::new(r#"import "@lib/he<caret>""#)
        .manifest(
            r#"
                [import-mappings]
                lib = "/workspace/lib"
            "#,
        )
        .file("lib/helpers.tolk", "fun helper() {}")
        .check_applied("helpers", expect![[r#"import "@lib/helpers<caret>""#]]);
}

#[test]
fn hides_internal_double_underscore_files() {
    // Generated implementation files stay out of import completion, while a single
    // leading underscore remains a normal user-visible file name.
    CompletionTest::new(r#"import "<caret>""#)
        .file("__impl_emul.tolk", "fun internal() {}")
        .file("_helpers.tolk", "fun helper() {}")
        .labels(&["__impl_emul", "_helpers"])
        .check(expect![[r#"
            label     kind  detail  edit     text
            _helpers  File  .tolk   0:8-0:8  _helpers"#]]);
}
