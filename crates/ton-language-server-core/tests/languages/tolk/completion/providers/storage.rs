use super::support::CompletionTest;
use expect_test::expect;

#[test]
fn completes_storage_helpers_for_storage_struct() {
    // Top-level completion offers the combined storage/load/save template.
    CompletionTest::new("<caret>")
        .labels(&["storage"])
        .check(expect![[r#"
            label    kind     detail  edit     text
            storage  Snippet          0:0-0:0  struct ${1:Storage} {\n    $0\n}\n\nfun ${1:Storage}.load() {\n    return ${1:Storage}.fromCell(contract.getData());\n}\n\nfun ${1:Storage}.save(self) {\n    contract.setData(self.toCell());\n}"#]]);

    // The storage template is unavailable inside function bodies.
    CompletionTest::new("fun main() { <caret> }")
        .labels(&["storage"])
        .check(expect!["<none>"]);
}

#[test]
fn applies_storage_template() {
    // Applying storage creates the struct and both persistence helper methods.
    CompletionTest::new("stor<caret>").check_applied(
        "storage",
        expect![[r#"
            struct Storage<caret> {
    
            }

            fun Storage.load() {
                return Storage.fromCell(contract.getData());
            }

            fun Storage.save(self) {
                contract.setData(self.toCell());
            }"#]],
    );
}
