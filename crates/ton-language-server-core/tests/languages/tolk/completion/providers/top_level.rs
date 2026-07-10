use super::support::CompletionTest;
use expect_test::expect;

#[test]
fn completes_top_level_declarations_but_not_inside_functions() {
    let labels = [
        "import",
        "contract",
        "struct",
        "enum",
        "type",
        "const",
        "global",
        "fun",
        "inline fun",
        "inline_ref fun",
        "asm fun",
        "method fun",
        "static method fun",
        "get fun",
    ];

    // An empty file exposes every top-level declaration template.
    CompletionTest::new("<caret>")
        .labels(&labels)
        .check(expect![[r#"
            label              kind     detail  edit     text
            asm fun            Snippet          0:0-0:0  fun ${1:name}($2)$3 asm "$0"
            const              Snippet          0:0-0:0  const ${1:FOO}: ${2:int} = ${3:0}$0
            contract           Snippet          0:0-0:0  contract ${1:Name} {\n    author: "${2:}"\n    version: "${3:1.0.0}"\n    description: "${4:My TON contract}"\n    incomingMessages: ${5:AllowedMessages}\n    storage: ${6:Storage}\n}$0
            enum               Snippet          0:0-0:0  enum ${1:Name} {\n    $0\n}
            fun                Snippet          0:0-0:0  fun ${1:name}($2)$3 {\n    $0\n}
            get fun            Snippet          0:0-0:0  get fun ${1:name}($2)$3 {\n    $0\n}
            global             Snippet          0:0-0:0  global ${1:foo}: ${2:int}$0
            import             Snippet          0:0-0:0  import "$1"$0
            inline fun         Snippet          0:0-0:0  @inline\nfun ${1:name}($2)$3 {\n    $0\n}
            inline_ref fun     Snippet          0:0-0:0  @inline_ref\nfun ${1:name}($2)$3 {\n    $0\n}
            method fun         Snippet          0:0-0:0  fun ${1:Foo}.${2:name}(${3:self}$4)$5 {\n    $0\n}
            static method fun  Snippet          0:0-0:0  fun ${1:Foo}.${2:name}($3)$4 {\n    $0\n}
            struct             Snippet          0:0-0:0  struct ${1:Name} {\n    $0\n}
            type               Snippet          0:0-0:0  type ${1:Int} = ${2:int}$0"#]]);

    // Top-level templates are suppressed inside a function body.
    CompletionTest::new("fun main() { <caret> }")
        .labels(&["inline fun", "asm fun", "method fun", "get fun"])
        .check(expect!["<none>"]);
}

#[test]
fn completes_before_and_after_other_top_level_declarations() {
    // Templates remain available between an import and a later declaration.
    CompletionTest::new(
        r#"
            import "./messages"
            <caret>
            fun main() {}
        "#,
    )
    .labels(&["import", "struct", "fun"])
    .check(expect![[r#"
        label   kind     detail  edit     text
        fun     Snippet          1:0-1:0  fun ${1:name}($2)$3 {\n    $0\n}
        import  Snippet          1:0-1:0  import "$1"$0
        struct  Snippet          1:0-1:0  struct ${1:Name} {\n    $0\n}"#]]);

    // Templates also remain available after a completed declaration.
    CompletionTest::new(
        "
            struct Storage {}
            <caret>
        ",
    )
    .labels(&["struct", "fun", "contract"])
    .check(expect![[r#"
        label     kind     detail  edit     text
        contract  Snippet          1:0-1:0  contract ${1:Name} {\n    author: "${2:}"\n    version: "${3:1.0.0}"\n    description: "${4:My TON contract}"\n    incomingMessages: ${5:AllowedMessages}\n    storage: ${6:Storage}\n}$0
        fun       Snippet          1:0-1:0  fun ${1:name}($2)$3 {\n    $0\n}
        struct    Snippet          1:0-1:0  struct ${1:Name} {\n    $0\n}"#]]);
}

#[test]
fn completes_test_function_template_only_in_test_files() {
    // Test files receive the Acton get-method test template.
    CompletionTest::new("<caret>")
        .uri("file:///workspace/counter.test.tolk")
        .labels(&["get fun test"])
        .check(expect![[r#"
            label         kind     detail  edit     text
            get fun test  Snippet          0:0-0:0  get fun `test $1`() {$0}"#]]);

    // Ordinary source files do not receive the test-only template.
    CompletionTest::new("<caret>")
        .labels(&["get fun test"])
        .check(expect!["<none>"]);
}

#[test]
fn applies_top_level_templates() {
    // Import completion preserves quotes and selects the path placeholder.
    CompletionTest::new("imp<caret>").check_applied("import", expect![[r#"import "<caret>""#]]);

    // Struct completion creates a complete declaration body.
    CompletionTest::new("str<caret>").check_applied(
        "struct",
        expect![[r#"
            struct Name<caret> {
    
            }"#]],
    );

    // Constant completion expands its name, type, and initial value placeholders.
    CompletionTest::new("con<caret>").check_applied("const", expect!["const FOO<caret>: int = 0"]);

    // Global completion expands its name and type placeholders.
    CompletionTest::new("glo<caret>").check_applied("global", expect!["global foo<caret>: int"]);

    // Type completion expands an alias declaration.
    CompletionTest::new("typ<caret>").check_applied("type", expect!["type Int<caret> = int"]);

    // Assembly completion expands the function signature and assembly body.
    CompletionTest::new("asm<caret>")
        .check_applied("asm fun", expect![[r#"fun name<caret>() asm """#]]);

    // Plain function completion expands a complete declaration.
    CompletionTest::new("fun<caret>").check_applied(
        "fun",
        expect![[r#"
            fun name<caret>() {

            }"#]],
    );

    // Inline function completion includes the annotation on the preceding line.
    CompletionTest::new("inline<caret>").check_applied(
        "inline fun",
        expect![[r#"
            @inline
            fun name<caret>() {

            }"#]],
    );

    // Inline-ref function completion includes its annotation on the preceding line.
    CompletionTest::new("inline_ref<caret>").check_applied(
        "inline_ref fun",
        expect![[r#"
            @inline_ref
            fun name<caret>() {

            }"#]],
    );

    // Get-function completion expands a complete get method declaration.
    CompletionTest::new("get<caret>").check_applied(
        "get fun",
        expect![[r#"
            get fun name<caret>() {

            }"#]],
    );

    // Instance-method completion includes a receiver and self parameter.
    CompletionTest::new("method<caret>").check_applied(
        "method fun",
        expect![[r#"
            fun Foo<caret>.name(self) {

            }"#]],
    );

    // Static-method completion includes a receiver without self.
    CompletionTest::new("static<caret>").check_applied(
        "static method fun",
        expect![[r#"
            fun Foo<caret>.name() {

            }"#]],
    );

    // Contract completion expands all supported metadata fields.
    CompletionTest::new("con<caret>").check_applied(
        "contract",
        expect![[r#"
            contract Name<caret> {
                author: ""
                version: "1.0.0"
                description: "My TON contract"
                incomingMessages: AllowedMessages
                storage: Storage
            }"#]],
    );
}
