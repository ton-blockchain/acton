use expect_test::Expect;
use tolk_ast::SourceFile;
use tolkfmt::format_source;

pub(crate) fn check(code: &str, expect: Expect) {
    check_with_width(code, expect, 80)
}

pub(crate) fn check_with_width(code: &str, expect: Expect, width: usize) {
    check_code(code, expect, width, true)
}

pub(crate) fn check_without_trees(code: &str, expect: Expect) {
    check_with_width_without_trees(code, expect, 80)
}

pub(crate) fn check_with_width_without_trees(code: &str, expect: Expect, width: usize) {
    check_code(code, expect, width, false)
}

fn check_code(code: &str, expect: Expect, width: usize, check_trees: bool) {
    // unsafe { std::env::set_var("UPDATE_EXPECT", "1") }
    let res = format_source(code, width).unwrap();

    equal_format_code(expect, &res);
    equal_trees(code, &res, check_trees);
}

fn equal_format_code(expect: Expect, code: &str) {
    let res = code
        .lines()
        .map(|l| if l.trim().is_empty() { "" } else { l })
        .collect::<Vec<_>>()
        .join("\n");

    expect.assert_eq(&res);
}

fn equal_trees(old_code: &str, new_code: &str, check_trees: bool) {
    let old_tree = parse_tolk_code(old_code).unwrap_or("<error>".to_string());
    let new_tree = parse_tolk_code(new_code).unwrap_or("<error>".to_string());

    if check_trees {
        assert_eq!(old_tree, new_tree);
    } else {
        if old_tree == new_tree {
            assert!(
                false,
                "Checks for identical trees are ignored, even though the trees are identical",
            )
        }
    }
}

fn parse_tolk_code(source: &str) -> anyhow::Result<String> {
    let tree = tolk_parser::parser::parse(source)?;
    let source_file = SourceFile {
        tree,
        source: source.into(),
    };

    let root_node = source_file.tree.root_node();
    if root_node.has_error() {
        anyhow::bail!("Cannot format code with syntax error");
    }

    let root_sexp = root_node.to_sexp().replace(" (empty_statement)", "");
    Ok(root_sexp)
}
