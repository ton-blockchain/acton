use expect_test::Expect;

pub(crate) fn check(input: &str, expected: Expect) {
    check_with_width(input, expected, 80);
}

pub(crate) fn check_with_width(input: &str, expected: Expect, width: usize) {
    let input = dedent(input);
    let actual = tolkfmt::format_source(&input, width).expect("formatting failed");
    expected.assert_eq(&actual);
}

fn dedent(input: &str) -> String {
    let lines: Vec<&str> = input.lines().collect();

    // Find the minimum indentation (excluding empty lines)
    let min_indent = lines
        .iter()
        .filter(|line| !line.trim().is_empty())
        .map(|line| line.len() - line.trim_start().len())
        .min()
        .unwrap_or(0);

    // Remove the common indentation
    lines
        .iter()
        .map(|line| {
            if line.trim().is_empty() {
                ""
            } else {
                &line[min_indent..]
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}
