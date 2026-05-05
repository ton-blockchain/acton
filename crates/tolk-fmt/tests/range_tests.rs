use expect_test::{Expect, expect};
use tolk_fmt::{FormatOptions, FormatPosition, FormatRange, format_source};

const SELECTION_START: &str = "<selection>";
const SELECTION_END: &str = "</selection>";

fn check_selection(marked_code: &str, width: usize, expect: Expect) {
    let (code, range) = parse_selection(marked_code);
    let formatted = format_source(
        &code,
        FormatOptions {
            width,
            range: Some(range),
            ..Default::default()
        },
    )
    .expect("range formatting should succeed");

    let normalized = formatted
        .lines()
        .map(|line| if line.trim().is_empty() { "" } else { line })
        .collect::<Vec<_>>()
        .join("\n");

    expect.assert_eq(&normalized);
}

fn parse_selection(marked_code: &str) -> (String, FormatRange) {
    let mut code = String::with_capacity(marked_code.len());
    let mut line = 0;
    let mut character = 0;
    let mut start = None;
    let mut end = None;
    let mut index = 0;

    while index < marked_code.len() {
        let rest = &marked_code[index..];
        if rest.starts_with(SELECTION_START) {
            assert!(
                start.is_none(),
                "only one selection start marker is supported"
            );
            start = Some(FormatPosition { line, character });
            index += SELECTION_START.len();
            continue;
        }
        if rest.starts_with(SELECTION_END) {
            assert!(end.is_none(), "only one selection end marker is supported");
            end = Some(FormatPosition { line, character });
            index += SELECTION_END.len();
            continue;
        }

        let Some(ch) = rest.chars().next() else {
            break;
        };
        code.push(ch);
        if ch == '\n' {
            line += 1;
            character = 0;
        } else {
            character += ch.len_utf8();
        }
        index += ch.len_utf8();
    }

    let Some(start) = start else {
        panic!("missing {SELECTION_START} marker");
    };
    let Some(end) = end else {
        panic!("missing {SELECTION_END} marker");
    };

    assert!(
        start.line < end.line || start.line == end.line && start.character <= end.character,
        "selection end must not be before selection start"
    );

    (code, FormatRange { start, end })
}

#[test]
fn test_range_formats_only_selected_statement() {
    check_selection(
        r"fun foo() {
    val   x   =   1;
    <selection>val   y   =   2;</selection>
    val   z   =   3;
}",
        80,
        expect![[r"
            fun foo() {
                val   x   =   1;
                val y = 2;
                val   z   =   3;
            }"]],
    );
}

#[test]
fn test_range_formats_function_signature_without_reformatting_body() {
    check_selection(
        r"<selection>fun foo(){</selection>
    val   x   =   1;
    val   y   =   2;
}",
        80,
        expect![[r"
            fun foo() {
                val   x   =   1;
                val   y   =   2;
            }"]],
    );
}

#[test]
fn test_range_formats_selected_expression() {
    check_selection(
        r"fun foo() {
    val x = <selection>1+2*3</selection>;
    val y    =      2;
}",
        30,
        expect![[r"
            fun foo() {
                val x = 1 + 2 * 3;
                val y    =      2;
            }"]],
    );
}

#[test]
fn test_range_preserves_unselected_top_level_declarations() {
    check_selection(
        r"type MyType    =   int

<selection>struct MyStruct {
    field1: int;
    field2: string;
}</selection>

fun foo() {
    val   x   =   1;
}",
        80,
        expect![[r"
            type MyType    =   int

            struct MyStruct {
                field1: int
                field2: string
            }

            fun foo() {
                val   x   =   1;
            }"]],
    );
}

#[test]
fn test_range_preserves_comment_only_file() {
    check_selection(
        r"<selection>// file comment
// second comment</selection>",
        80,
        expect![[r"
            // file comment
            // second comment"]],
    );
}

#[test]
fn test_range_preserves_file_header_comment() {
    check_selection(
        r"// file header
// second header line

fun foo() {
    <selection>val   x   =   1;</selection>
    val   y   =   2;
}",
        80,
        expect![[r"
            // file header
            // second header line

            fun foo() {
                val x = 1;
                val   y   =   2;
            }"]],
    );
}

#[test]
fn test_range_formats_multiple_selected_statements() {
    check_selection(
        r"fun foo() {
    <selection>val   x   =   1;
    val   y   =   2;</selection>
    val   z   =   3;
}",
        80,
        expect![[r"
            fun foo() {
                val x = 1;
                val y = 2;
                val   z   =   3;
            }"]],
    );
}

#[test]
fn test_range_end_boundary_does_not_format_next_statement() {
    check_selection(
        r"fun foo() {
    <selection>val   x   =   1;
</selection>    val   y   =   2;
}",
        80,
        expect![[r"
            fun foo() {
                val x = 1;
                val   y   =   2;
            }"]],
    );
}

#[test]
fn test_range_preserves_import_order() {
    check_selection(
        r#"import "./b"
<selection>import   "./a"</selection>
fun foo() {}"#,
        80,
        expect![[r#"
            import "./b"
            import "./a"
            fun foo() {}"#]],
    );
}

#[test]
fn test_range_formats_only_selected_top_level_function() {
    check_selection(
        r"fun untouched(){val   a=1;}
<selection>fun selected(){val   b=2;}</selection>",
        80,
        expect![[r"
            fun untouched(){val   a=1;}
            fun selected() {
                val b = 2;
            }"]],
    );
}

#[test]
fn test_range_formats_parameter_list() {
    check_selection(
        r"<selection>fun foo(a:int,b:slice){</selection>
    val   x   =   1;
}",
        80,
        expect![[r"
            fun foo(a: int, b: slice) {
                val   x   =   1;
            }"]],
    );
}

#[test]
fn test_range_formats_function_call() {
    check_selection(
        r"fun foo() {
    val x = <selection>foo(1+2,3+4)</selection>;
    val y    =      2;
}",
        80,
        expect![[r"
            fun foo() {
                val x = foo(1 + 2, 3 + 4);
                val y    =      2;
            }"]],
    );
}

#[test]
fn test_range_formats_long_function_call_with_width() {
    check_selection(
        r"fun foo() {
    <selection>foo(veryLongArgument1, veryLongArgument2, veryLongArgument3)</selection>;
    val   y   =   2;
}",
        30,
        expect![[r"
            fun foo() {
                foo(
                    veryLongArgument1,
                    veryLongArgument2,
                    veryLongArgument3,
                );
                val   y   =   2;
            }"]],
    );
}

#[test]
fn test_range_formats_if_condition() {
    check_selection(
        r"fun foo() {
    if (<selection>a&&b||c</selection>) {
        val   x   =   1;
    }
}",
        80,
        expect![[r"
            fun foo() {
                if (a && b || c) {
                    val   x   =   1;
                }
            }"]],
    );
}

#[test]
fn test_range_formats_match_arms() {
    check_selection(
        r"fun foo() {
    match (x) {
        <selection>1=>return 10,
        2=>throw 20,
        else=>return 0</selection>
    }
    val   y   =   2;
}",
        80,
        expect![[r"
            fun foo() {
                match (x) {
                    1 => return 10,
                    2 => throw 20,
                    else => return 0,
                }
                val   y   =   2;
            }"]],
    );
}

#[test]
fn test_range_formats_only_selected_multiline_match_arm() {
    check_selection(
        r"fun foo() {
    match (x) {
        1=> {
        var a=1;
        return a;
        }
        <selection>2=> {
        var b=2;
        return b+1;
        }</selection>
        else=> {
        return 0;
        }
    }
}",
        80,
        expect![[r"
            fun foo() {
                match (x) {
                    1=> {
                    var a=1;
                    return a;
                    }
                    2 => {
                        var b = 2;
                        return b + 1;
                    }
                    else=> {
                    return 0;
                    }
                }
            }"]],
    );
}

#[test]
fn test_range_formats_object_literal() {
    check_selection(
        r"fun foo() {
    val x = <selection>Foo { a:1,b:2 }</selection>;
    val   y   =   2;
}",
        80,
        expect![[r"
            fun foo() {
                val x = Foo {
                    a: 1,
                    b: 2,
                };
                val   y   =   2;
            }"]],
    );
}

#[test]
fn test_range_formats_tuple_destructuring_statement() {
    check_selection(
        r"fun foo() {
    <selection>val [a,b]=[1,2];</selection>
    val   y   =   2;
}",
        80,
        expect![[r"
            fun foo() {
                val [a, b] = [1, 2];
                val   y   =   2;
            }"]],
    );
}

#[test]
fn test_range_formats_type_alias() {
    check_selection(
        r"<selection>type MyType=int|slice</selection>
fun foo(){val   x=1;}",
        80,
        expect![[r"
            type MyType = int | slice
            fun foo(){val   x=1;}"]],
    );
}

#[test]
fn test_range_respects_fmt_ignore_inside_selection() {
    check_selection(
        r"fun foo() {
    // fmt-ignore
    <selection>val   x   =   1;</selection>
    val   y   =   2;
}",
        80,
        expect![[r"
            fun foo() {
                // fmt-ignore
                val   x   =   1;
                val   y   =   2;
            }"]],
    );
}
