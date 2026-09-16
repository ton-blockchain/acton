#![cfg(test)]
use expect_test::Expect;
use tolk_fmt::{FormatOptions, format_source};

#[allow(dead_code)]
pub(crate) fn check(code: &str, expect: Expect) {
    check_with_width(code, expect, 80);
}

#[allow(dead_code)]
pub(crate) fn check_with_width(code: &str, expect: Expect, width: usize) {
    check_code(code, expect, width, true, FormatOptions::default());
}

#[allow(dead_code)]
pub(crate) fn check_without_trees(code: &str, expect: Expect) {
    check_with_width_without_trees(code, expect, 80);
}

#[allow(dead_code)]
pub(crate) fn check_with_width_without_trees(code: &str, expect: Expect, width: usize) {
    check_code(code, expect, width, false, FormatOptions::default());
}

#[allow(dead_code)]
pub(crate) fn check_without_trees_with_import_group_separators(
    code: &str,
    expect: Expect,
    separate_import_groups: bool,
) {
    check_with_width_and_options(
        code,
        expect,
        80,
        false,
        FormatOptions {
            separate_import_groups,
            ..FormatOptions::default()
        },
    );
}

#[allow(dead_code)]
pub(crate) fn check_with_import_group_separators(
    code: &str,
    expect: Expect,
    separate_import_groups: bool,
) {
    check_with_width_and_options(
        code,
        expect,
        80,
        true,
        FormatOptions {
            separate_import_groups,
            ..FormatOptions::default()
        },
    );
}

fn check_with_width_and_options(
    code: &str,
    expect: Expect,
    width: usize,
    check_trees: bool,
    options: FormatOptions,
) {
    check_code(code, expect, width, check_trees, options);
}

fn check_code(code: &str, expect: Expect, width: usize, check_trees: bool, options: FormatOptions) {
    let options = FormatOptions { width, ..options };
    let res = format_source(code, options).unwrap();

    equal_format_code(&expect, &res);
    equal_trees(code, &res, check_trees);

    let repeated = format_source(&res, options).expect("formatted source must remain valid Tolk");
    assert_eq!(res, repeated, "formatting must be stable after one pass");
}

fn equal_format_code(expect: &Expect, code: &str) {
    let res = code.lines().collect::<Vec<_>>().join("\n");

    expect.assert_eq(&res);
}

fn equal_trees(old_code: &str, new_code: &str, check_trees: bool) {
    let old_tree = parse_tolk_code(old_code).expect("test input must be valid Tolk");
    let new_tree = parse_tolk_code(new_code).expect("formatted source must be valid Tolk");

    if check_trees {
        assert_eq!(old_tree, new_tree);
    } else if old_tree == new_tree {
        panic!("Checks for identical trees are ignored, even though the trees are identical",)
    }
}

fn parse_tolk_code(source: &str) -> anyhow::Result<(String, Vec<String>)> {
    let source_file = tolk_syntax::parse(source)?;
    let root_node = source_file.root_node();
    if root_node.has_error() {
        anyhow::bail!("Cannot format code with syntax error");
    }

    let root_sexp = root_node.to_sexp().replace(" (empty_statement)", "");
    // CST shapes omit literal values. Compare their text too so formatting cannot
    // change a string or number while keeping the same syntax-tree shape.
    let mut literals = vec![];
    let mut nodes = vec![root_node];
    while let Some(node) = nodes.pop() {
        if matches!(
            node.kind(),
            "string_literal" | "number_literal" | "boolean_literal" | "null_literal"
        ) {
            literals.push(source[node.byte_range()].to_owned());
        } else {
            nodes.extend(node.children(&mut node.walk()));
        }
    }
    // Import sorting changes the order of path literals, but must preserve their contents.
    literals.sort();
    Ok((root_sexp, literals))
}
