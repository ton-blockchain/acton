use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const IO_IMPORTS: &str = r#"
import "../../lib/io"
"#;

#[test]
fn println_formats_bits264_void_map_entries() {
    let source = format!(
        "{IO_IMPORTS}\n{}",
        r#"
get fun `test println formats bits264 void map entries`() {
    val key = "111111111111111111111111111111111111111111111111111111111111111111".hexToSlice() as bits264;
    var plugins = createEmptyMap<bits264, void>();
    plugins.set(key);

    println("plugins={}", plugins);
}
"#
    );

    ProjectBuilder::new("println-formats-bits264-void-map-entries")
        .test_file("println_bits_map_keys", &source)
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/println_formats_bits_map_keys/println_formats_bits264_void_map_entries.stdout.txt",
        );
}
