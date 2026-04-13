use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

fn parse_int_script(input: &str) -> String {
    format!(
        r#"
import "../../lib/vm/vm"
import "../../lib/io"

fun main() {{
    val n = vm.parseInt("{input}");
    println("parsed={{}}", n);
}}
"#,
    )
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
