use expect_test::Expect;
use tolkfmt::format_source;

pub fn check(code: &str, expect: Expect) {
    check_with_width(code, expect, 80)
}

pub fn check_with_width(code: &str, expect: Expect, width: usize) {
    // unsafe { std::env::set_var("UPDATE_EXPECT", "1") }

    let res = format_source(code, width).unwrap();

    let res = res
        .lines()
        .map(|l| if l.trim().is_empty() { "" } else { l })
        .collect::<Vec<_>>()
        .join("\n");

    expect.assert_eq(&res);
}
