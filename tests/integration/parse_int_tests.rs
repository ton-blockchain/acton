use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;
use std::fmt::Write;

fn parse_int_script(input: &str) -> String {
    format!(
        r#"
import "../../lib/fmt"
import "../../lib/io"

fun main() {{
    val n = parseInt("{input}");
    println("parsed={{}}", n);
}}
"#,
    )
}

#[test]
fn parse_int_and_prompt_int_accept_integer_literals() {
    ProjectBuilder::new("parse-int-literals")
        .script_file(
            "integer_literals",
            r#"
import "../../lib/fmt"
import "../../lib/io"
import "../../lib/prompts"

fun main() {
    val publicKey = promptInt(
        "DEX owner public key (uint256; decimal or 0x-prefixed hex):",
        "0x3e4918f7faa48afcb4569138b3cc291b4e76e59aba9c72d2fef1e8d586c5cf3e",
        "0x...",
    ) as uint256;
    println("publicKey={:x}", publicKey);

    val inputs: array<string> = [
        "0", "42", "+42", "-42", " 42 ", "1_000",
        "0xff", "0xAbCd", "+0xff", "-0xff", "  -0xFF  ", "0xFF_FF",
        "0b101010", "-0b101010",
        "0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
    ];
    var i = 0;
    repeat (inputs.size()) {
        val input = inputs.get(i);
        println("{}: parseInt={}, promptInt={}", input, parseInt(input), promptInt("Integer", input));
        i += 1;
    }
}
"#,
        )
        .build()
        .acton()
        .script("scripts/integer_literals.tolk")
        .run()
        .success()
        .assert_snapshot_matches("integration/snapshots/parse_int/integer_literals.stdout.txt");
}

#[test]
fn parse_int_and_prompt_int_reject_malformed_literals() {
    let mut code = String::from("import \"../../lib/fmt\"\nimport \"../../lib/prompts\"\n");
    for (case, input) in [
        ("missing hex digits", "0x"),
        ("invalid hex digit", "0xgg"),
        ("sign after prefix", "0x-1"),
        ("multiple signs", "+-0x1"),
        ("invalid binary digit", "0b102"),
        ("empty input", ""),
    ] {
        write!(
            code,
            r#"
get fun `test parseInt {case}`() {{ parseInt("{input}"); }}
get fun `test promptInt {case}`() {{ promptInt("Integer", "{input}"); }}
"#,
        )
        .expect("failed to write malformed literal test");
    }

    ProjectBuilder::new("parse-int-malformed-literals")
        .test_file("malformed_literals", &code)
        .build()
        .acton()
        .test()
        .run()
        .failure()
        .assert_snapshot_matches("integration/snapshots/parse_int/malformed_literals.stdout.txt");
}

#[test]
fn parse_int_rejects_non_numeric_string() {
    ProjectBuilder::new("parse-int-non-numeric")
        .script_file("use_parse_int", &parse_int_script("abc"))
        .build()
        .acton()
        .script("scripts/use_parse_int.tolk")
        .run()
        .failure()
        .assert_snapshot_matches(
            "integration/snapshots/parse_int/rejects_non_numeric_string.stdout.txt",
        );
}

#[test]
fn parse_int_rejects_empty_string() {
    ProjectBuilder::new("parse-int-empty")
        .script_file("use_parse_int", &parse_int_script(""))
        .build()
        .acton()
        .script("scripts/use_parse_int.tolk")
        .run()
        .failure()
        .assert_snapshot_matches("integration/snapshots/parse_int/rejects_empty_string.stdout.txt");
}

#[test]
fn parse_int_rejects_float() {
    ProjectBuilder::new("parse-int-float")
        .script_file("use_parse_int", &parse_int_script("3.14"))
        .build()
        .acton()
        .script("scripts/use_parse_int.tolk")
        .run()
        .failure()
        .assert_snapshot_matches("integration/snapshots/parse_int/rejects_float.stdout.txt");
}
