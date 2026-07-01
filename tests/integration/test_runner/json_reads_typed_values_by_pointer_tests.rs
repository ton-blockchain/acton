use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const METADATA_JSON: &str = r#"{
    "name": "Demo",
    "decimals": 9,
    "mintable": true,
    "frozen": false,
    "big": "123456789012345678901234567890",
    "hex": "0x1a",
    "ratio": 1.5,
    "token": { "symbol": "DEMO" },
    "items": [10, 20, 30]
}"#;

const JSON_IMPORTS: &str = r#"
import "../../lib/fs"
import "../../lib/json"
import "../../lib/testing/expect"
"#;

fn run_case(project_name: &str, test_body: &str) {
    let test_code = format!("{JSON_IMPORTS}\n{test_body}\n");

    ProjectBuilder::new(project_name)
        .test_file("json_behavior", &test_code)
        .raw_file("fixtures/metadata.json", METADATA_JSON)
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1);
}

#[test]
fn json_get_int_reads_numbers_strings_and_hex_and_rejects_floats() {
    run_case(
        "z-stdlib-json-get-int",
        r#"
get fun `test z stdlib json get int`() {
    val src = fs.readFile("fixtures/metadata.json")!;

    expect(json.getInt(src, "/decimals")).toEqual(9);
    expect(json.getInt(src, "/items/1")).toEqual(20);
    expect(json.getInt(src, "/hex")).toEqual(26);
    expect(json.getInt(src, "/big")).toEqual(123456789012345678901234567890);

    expect(json.getInt(src, "/ratio")).toBeNull();
    expect(json.getInt(src, "/name")).toBeNull();
    expect(json.getInt(src, "/missing")).toBeNull();
}
"#,
    );
}

#[test]
fn json_get_string_reads_strings_and_nested_paths() {
    run_case(
        "z-stdlib-json-get-string",
        r#"
get fun `test z stdlib json get string`() {
    val src = fs.readFile("fixtures/metadata.json")!;

    expect(json.getString(src, "/name")!).toEqual("Demo");
    expect(json.getString(src, "/token/symbol")!).toEqual("DEMO");

    expect(json.getString(src, "/decimals")).toBeNull();
    expect(json.getString(src, "/missing")).toBeNull();
}
"#,
    );
}

#[test]
fn json_get_bool_distinguishes_present_false_from_null() {
    run_case(
        "z-stdlib-json-get-bool",
        r#"
get fun `test z stdlib json get bool`() {
    val src = fs.readFile("fixtures/metadata.json")!;

    expect(json.getBool(src, "/mintable")!).toEqual(true);

    val frozen = json.getBool(src, "/frozen");
    expect(frozen).toBeNotNull();
    expect(frozen!).toEqual(false);

    expect(json.getBool(src, "/decimals")).toBeNull();
    expect(json.getBool(src, "/missing")).toBeNull();
}
"#,
    );
}

#[test]
fn json_exists_and_invalid_source_return_expected_flags() {
    run_case(
        "z-stdlib-json-exists-and-invalid",
        r#"
get fun `test z stdlib json exists and invalid source`() {
    val src = fs.readFile("fixtures/metadata.json")!;

    expect(json.exists(src, "/token")).toEqual(true);
    expect(json.exists(src, "/frozen")).toEqual(true);
    expect(json.exists(src, "/missing")).toEqual(false);

    // Invalid JSON collapses to null / false rather than throwing.
    expect(json.getString("this is not json", "/x")).toBeNull();
    expect(json.exists("this is not json", "/x")).toEqual(false);
}
"#,
    );
}
