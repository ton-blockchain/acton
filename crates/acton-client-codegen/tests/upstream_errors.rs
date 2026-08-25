use acton_client_codegen::generate;
use expect_test::expect;

const ERR_CONT_ON_STACK_1: &str =
    include_str!("fixtures/upstream-errors/err-cont-on-stack-1.abi.json");
const ERR_CONT_ON_STACK_2: &str =
    include_str!("fixtures/upstream-errors/err-cont-on-stack-2.abi.json");
const ERR_INVALID_MAP_KEY_1: &str =
    include_str!("fixtures/upstream-errors/err-invalid-map-key-1.abi.json");
const ERR_INVALID_MAP_KEY_2: &str =
    include_str!("fixtures/upstream-errors/err-invalid-map-key-2.abi.json");

fn assert_substrings(error: &str, expected_substrings: &[&str]) {
    for expected in expected_substrings {
        assert!(
            error.contains(expected),
            "expected generated error to contain {expected:?}, got {error:?}"
        );
    }
}

#[test]
fn err_cont_on_stack_1_matches_upstream() {
    let error = generate(ERR_CONT_ON_STACK_1)
        .expect_err("continuation parameter must fail")
        .to_string();
    expect![
        "Error while generating get method 'acceptingContinuation': [NotSupportedTypeOnStack] 'd.n' can not be used in get methods, because it contains 'continuation'"
    ]
    .assert_eq(&error);
    assert_substrings(
        &error,
        &[
            "Error while generating get method 'acceptingContinuation'",
            "'d.n' can not be used in get methods, because it contains 'continuation'",
        ],
    );
}

#[test]
fn err_cont_on_stack_2_matches_upstream() {
    let error = generate(ERR_CONT_ON_STACK_2)
        .expect_err("continuation result must fail")
        .to_string();
    expect![
        "Error while generating get method 'returningContinuation': [NotSupportedTypeOnStack] 'result[1]' can not be used in get methods, because it contains 'continuation'"
    ]
    .assert_eq(&error);
    assert_substrings(
        &error,
        &[
            "Error while generating get method 'returningContinuation'",
            "'result[1]' can not be used in get methods, because it contains 'continuation'",
        ],
    );
}

#[test]
fn err_invalid_map_key_1_matches_upstream() {
    let error = generate(ERR_INVALID_MAP_KEY_1)
        .expect_err("alias map key must fail")
        .to_string();
    expect![
        "Error while generating alias 'HasUnsupportedMapKey': [NonStandardDictKey] 'HasUnsupportedMapKey' is 'map<JustInt32, ...>': such a non-standard map key can not be handled by @ton/core library"
    ]
    .assert_eq(&error);
    assert_substrings(
        &error,
        &[
            "Error while generating alias 'HasUnsupportedMapKey'",
            "[NonStandardDictKey] 'HasUnsupportedMapKey' is 'map<JustInt32, ...>'",
            "such a non-standard map key can not be handled by @ton/core library",
        ],
    );
}

#[test]
fn err_invalid_map_key_2_matches_upstream() {
    let error = generate(ERR_INVALID_MAP_KEY_2)
        .expect_err("field map key must fail")
        .to_string();
    expect![
        "Error while generating struct 'Demo': [NonStandardDictKey] 'Demo.errM' is 'map<Point, ...>': such a non-standard map key can not be handled by @ton/core library"
    ]
    .assert_eq(&error);
    assert_substrings(
        &error,
        &[
            "Error while generating struct 'Demo'",
            "[NonStandardDictKey] 'Demo.errM' is 'map<Point, ...>'",
            "such a non-standard map key can not be handled by @ton/core library",
        ],
    );
}
