use crate::support::TestOutputExt;
use crate::support::project::{Project, ProjectBuilder};

const FS_IMPORTS: &str = r#"
import "../../lib/fs"
import "../../lib/testing/expect"
"#;

fn build_fs_project(project_name: &str, test_code: &str) -> Project {
    let full_code = format!("{FS_IMPORTS}\n{test_code}\n");
    ProjectBuilder::new(project_name)
        .test_file("fs_new_api", &full_code)
        .build()
}

#[test]
fn fs_read_bytes_supports_binary_content_and_path_normalization() {
    let project = build_fs_project(
        "w-stdlib-fs-read-bytes-path-normalization",
        r#"
get fun `test-fs-read-bytes-supports-binary-and-normalized-paths`() {
    val direct = fs.readBytes("fixtures/input.bin");
    val viaDot = fs.readBytes("./fixtures/input.bin");
    val viaParent = fs.readBytes("fixtures/../fixtures/input.bin");

    expect(direct).toBeNotNull();
    expect(viaDot).toBeNotNull();
    expect(viaParent).toBeNotNull();

    var a = direct!;
    var b = viaDot!;
    var c = viaParent!;

    expect(a.remainingBitsCount()).toEqual(32);
    expect(b.remainingBitsCount()).toEqual(32);
    expect(c.remainingBitsCount()).toEqual(32);

    expect(a.loadUint(8)).toEqual(0x00);
    expect(a.loadUint(8)).toEqual(0xFF);
    expect(a.loadUint(8)).toEqual(0x80);
    expect(a.loadUint(8)).toEqual(0x7A);
}
"#,
    );

    let fixtures_dir = project.path().join("fixtures");
    std::fs::create_dir_all(&fixtures_dir).expect("Failed to create fixtures directory");
    std::fs::write(fixtures_dir.join("input.bin"), [0x00, 0xFF, 0x80, 0x7A])
        .expect("Failed to write binary fixture");

    project.acton().test().run().success().assert_passed(1);
}

#[test]
fn fs_read_bytes_handles_empty_large_missing_and_directory_paths() {
    let project = build_fs_project(
        "w-stdlib-fs-read-bytes-corner-cases",
        r#"
get fun `test-fs-read-bytes-corner-cases`() {
    val empty = fs.readBytes("fixtures/empty.bin");
    expect(empty).toBeNotNull();
    expect(empty!.remainingBitsCount()).toEqual(0);

    val large = fs.readBytes("fixtures/large.bin");
    expect(large).toBeNotNull();
    var largeSlice = large!;
    expect(largeSlice.remainingBitsCount()).toEqual(1008);
    expect(largeSlice.remainingRefsCount()).toEqual(1);

    var i = 0;
    while (i < 126) {
        expect(largeSlice.loadUint(8)).toEqual(i);
        i += 1;
    }
    expect(largeSlice.remainingBitsCount()).toEqual(0);
    expect(largeSlice.remainingRefsCount()).toEqual(1);
    var second = largeSlice.loadRef().beginParse();
    expect(second.remainingBitsCount()).toEqual(1008);
    expect(second.remainingRefsCount()).toEqual(1);

    var third = second.loadRef().beginParse();
    expect(third.remainingBitsCount()).toEqual(384);
    expect(third.remainingRefsCount()).toEqual(0);

    expect(fs.readBytes("fixtures/missing.bin")).toBeNull();
    expect(fs.readBytes("fixtures/dir")).toBeNull();
}
"#,
    );

    let fixtures_dir = project.path().join("fixtures");
    std::fs::create_dir_all(fixtures_dir.join("dir")).expect("Failed to create fixtures dir");
    std::fs::write(fixtures_dir.join("empty.bin"), []).expect("Failed to write empty fixture");
    let large: Vec<u8> = (0..300).map(|i| (i % 256) as u8).collect();
    std::fs::write(fixtures_dir.join("large.bin"), large).expect("Failed to write large fixture");

    project.acton().test().run().success().assert_passed(1);
}

#[test]
fn fs_write_string_writes_overwrites_and_handles_failures() {
    let project = build_fs_project(
        "w-stdlib-fs-write-string-corner-cases",
        r#"
get fun `test-fs-write-string-corner-cases`() {
    expect(fs.exists("written.txt")).toBeFalse();
    expect(fs.writeString("written.txt", "alpha\nbeta")).toBeTrue();
    expect(fs.exists("written.txt")).toBeTrue();

    val first = fs.readFile("written.txt");
    expect(first).toBeNotNull();
    expect(first!).toEqual("alpha\nbeta");

    expect(fs.writeString("written.txt", "updated")).toBeTrue();
    val updated = fs.readFile("written.txt");
    expect(updated).toBeNotNull();
    expect(updated!).toEqual("updated");

    expect(fs.writeString("empty.txt", "")).toBeTrue();
    val empty = fs.readFile("empty.txt");
    expect(empty).toBeNotNull();
    expect(empty!).toEqual("");

    expect(fs.writeString("missing-dir/written.txt", "x")).toBeFalse();
    expect(fs.exists("missing-dir/written.txt")).toBeFalse();
}
"#,
    );

    project.acton().test().run().success().assert_passed(1);

    let written = std::fs::read_to_string(project.path().join("written.txt"))
        .expect("written.txt must exist after fs.writeString");
    assert_eq!(written, "updated");
    let empty = std::fs::read_to_string(project.path().join("empty.txt"))
        .expect("empty.txt must exist after fs.writeString");
    assert_eq!(empty, "");
}

#[test]
fn fs_write_bytes_roundtrips_small_and_empty_payloads() {
    let project = build_fs_project(
        "w-stdlib-fs-write-bytes-small-and-empty",
        r#"
get fun `test-fs-write-bytes-small-and-empty`() {
    val data = beginCell()
        .storeUint(0xDE, 8)
        .storeUint(0xAD, 8)
        .storeUint(0xBE, 8)
        .storeUint(0xEF, 8)
        .toSlice();

    expect(fs.writeBytes("written.bin", data)).toBeTrue();
    expect(fs.exists("written.bin")).toBeTrue();

    val roundtrip = fs.readBytes("written.bin");
    expect(roundtrip).toBeNotNull();
    var r = roundtrip!;
    expect(r.remainingBitsCount()).toEqual(32);
    expect(r.loadUint(8)).toEqual(0xDE);
    expect(r.loadUint(8)).toEqual(0xAD);
    expect(r.loadUint(8)).toEqual(0xBE);
    expect(r.loadUint(8)).toEqual(0xEF);

    val empty = beginCell().toSlice();
    expect(fs.writeBytes("empty.bin", empty)).toBeTrue();
    val emptyRoundtrip = fs.readBytes("empty.bin");
    expect(emptyRoundtrip).toBeNotNull();
    expect(emptyRoundtrip!.remainingBitsCount()).toEqual(0);
}
"#,
    );

    project.acton().test().run().success().assert_passed(1);

    let written_bytes =
        std::fs::read(project.path().join("written.bin")).expect("written.bin must be created");
    assert_eq!(written_bytes, [0xDE, 0xAD, 0xBE, 0xEF]);
    let empty_bytes =
        std::fs::read(project.path().join("empty.bin")).expect("empty.bin must be created");
    assert!(empty_bytes.is_empty());
}

#[test]
fn fs_write_bytes_handles_large_payload_and_rejects_invalid_slices() {
    let project = build_fs_project(
        "w-stdlib-fs-write-bytes-corner-cases",
        r#"
get fun `test-fs-write-bytes-corner-cases`() {
    val large = fs.readBytes("fixtures/large-input.bin");
    expect(large).toBeNotNull();

    expect(fs.writeBytes("large.bin", large!)).toBeTrue();
    expect(fs.exists("large.bin")).toBeTrue();

    val roundtrip = fs.readBytes("large.bin");
    expect(roundtrip).toBeNotNull();
    var first = roundtrip!;
    expect(first.remainingBitsCount()).toEqual(1008);
    expect(first.remainingRefsCount()).toEqual(1);

    var second = first.loadRef().beginParse();
    expect(second.remainingBitsCount()).toEqual(592);
    expect(second.remainingRefsCount()).toEqual(0);

    val misaligned = beginCell().storeUint(1, 1).toSlice();
    expect(fs.writeBytes("misaligned.bin", misaligned)).toBeFalse();
    expect(fs.exists("misaligned.bin")).toBeFalse();

    val invalidRefs = beginCell()
        .storeRef(beginCell().storeUint(1, 8).endCell())
        .storeRef(beginCell().storeUint(2, 8).endCell())
        .toSlice();
    expect(fs.writeBytes("invalid-refs.bin", invalidRefs)).toBeTrue();
    expect(fs.exists("invalid-refs.bin")).toBeTrue();

    val invalidRoundtrip = fs.readBytes("invalid-refs.bin");
    expect(invalidRoundtrip).toBeNotNull();
    var invalidSlice = invalidRoundtrip!;
    expect(invalidSlice.remainingBitsCount()).toEqual(8);
    expect(invalidSlice.loadUint(8)).toEqual(1);

    expect(fs.writeBytes("missing-dir/large.bin", large!)).toBeFalse();
    expect(fs.exists("missing-dir/large.bin")).toBeFalse();
}
"#,
    );

    let fixtures_dir = project.path().join("fixtures");
    std::fs::create_dir_all(&fixtures_dir).expect("Failed to create fixtures directory");
    std::fs::write(fixtures_dir.join("large-input.bin"), [0xAB; 200])
        .expect("Failed to write large-input fixture");

    project.acton().test().run().success().assert_passed(1);

    let large = std::fs::read(project.path().join("large.bin")).expect("large.bin must exist");
    assert_eq!(large.len(), 200);
    assert!(large.iter().all(|byte| *byte == 0xAB));
    assert!(!project.path().join("misaligned.bin").exists());
    let invalid_refs =
        std::fs::read(project.path().join("invalid-refs.bin")).expect("invalid-refs.bin exists");
    assert_eq!(invalid_refs, [0x01]);
}

#[test]
fn fs_read_bytes_boundary_sizes_126_and_127_bytes() {
    let project = build_fs_project(
        "w-stdlib-fs-read-bytes-boundary-sizes",
        r#"
get fun `test-fs-read-bytes-boundary-sizes`() {
    val bytes126 = fs.readBytes("fixtures/b126.bin");
    expect(bytes126).toBeNotNull();
    var s126 = bytes126!;
    expect(s126.remainingBitsCount()).toEqual(1008);
    expect(s126.remainingRefsCount()).toEqual(0);
    expect(s126.loadUint(8)).toEqual(0xAA);

    val bytes127 = fs.readBytes("fixtures/b127.bin");
    expect(bytes127).toBeNotNull();
    var s127 = bytes127!;
    expect(s127.remainingBitsCount()).toEqual(1008);
    expect(s127.remainingRefsCount()).toEqual(1);
    expect(s127.loadUint(8)).toEqual(0xBB);

    var tail = s127.loadRef().beginParse();
    expect(tail.remainingBitsCount()).toEqual(8);
    expect(tail.remainingRefsCount()).toEqual(0);
    expect(tail.loadUint(8)).toEqual(0xBB);
}
"#,
    );

    let fixtures_dir = project.path().join("fixtures");
    std::fs::create_dir_all(&fixtures_dir).expect("Failed to create fixtures directory");
    std::fs::write(fixtures_dir.join("b126.bin"), [0xAA; 126]).expect("Failed to write b126");
    std::fs::write(fixtures_dir.join("b127.bin"), [0xBB; 127]).expect("Failed to write b127");

    project.acton().test().run().success().assert_passed(1);
}

#[test]
fn fs_read_bytes_reads_non_utf8_data_that_read_file_cannot_decode() {
    let project = build_fs_project(
        "w-stdlib-fs-read-bytes-vs-read-file-non-utf8",
        r#"
get fun `test-fs-read-bytes-vs-read-file-non-utf8`() {
    val bytes = fs.readBytes("fixtures/non-utf8.bin");
    val text = fs.readFile("fixtures/non-utf8.bin");

    expect(bytes).toBeNotNull();
    expect(text).toBeNull();

    var s = bytes!;
    expect(s.remainingBitsCount()).toEqual(64);
    expect(s.loadUint(8)).toEqual(0x00);
    expect(s.loadUint(8)).toEqual(0xFF);
    expect(s.loadUint(8)).toEqual(0x80);
    expect(s.loadUint(8)).toEqual(0x10);
}
"#,
    );

    let fixtures_dir = project.path().join("fixtures");
    std::fs::create_dir_all(&fixtures_dir).expect("Failed to create fixtures directory");
    std::fs::write(
        fixtures_dir.join("non-utf8.bin"),
        [0x00, 0xFF, 0x80, 0x10, 0xB5, 0xEE, 0x9C, 0x72],
    )
    .expect("Failed to write non-utf8 fixture");

    project.acton().test().run().success().assert_passed(1);
}

#[test]
fn fs_write_methods_reject_directory_targets() {
    let project = build_fs_project(
        "w-stdlib-fs-write-reject-directory-target",
        r#"
get fun `test-fs-write-reject-directory-target`() {
    val payload = beginCell().storeUint(0xAB, 8).toSlice();

    expect(fs.exists("fixtures/dir-target")).toBeTrue();
    expect(fs.writeString("fixtures/dir-target", "x")).toBeFalse();
    expect(fs.writeBytes("fixtures/dir-target", payload)).toBeFalse();
    expect(fs.readBytes("fixtures/dir-target")).toBeNull();
}
"#,
    );

    let target_dir = project.path().join("fixtures/dir-target");
    std::fs::create_dir_all(&target_dir).expect("Failed to create directory target fixture");

    project.acton().test().run().success().assert_passed(1);
}

#[test]
fn fs_exists_reports_files_directories_and_normalized_paths() {
    let project = build_fs_project(
        "w-stdlib-fs-exists-corner-cases",
        r#"
get fun `test-fs-exists-corner-cases`() {
    expect(fs.exists("fixtures")).toBeTrue();
    expect(fs.exists("./fixtures")).toBeTrue();
    expect(fs.exists("fixtures/../fixtures")).toBeTrue();

    expect(fs.exists("fixtures/existing.txt")).toBeTrue();
    expect(fs.exists("./fixtures/existing.txt")).toBeTrue();
    expect(fs.exists("fixtures/../fixtures/existing.txt")).toBeTrue();

    expect(fs.exists("fixtures/missing.txt")).toBeFalse();
    expect(fs.exists("fixtures/../fixtures/missing.txt")).toBeFalse();
    expect(fs.exists("missing-dir")).toBeFalse();
}
"#,
    );

    let fixtures_dir = project.path().join("fixtures");
    std::fs::create_dir_all(&fixtures_dir).expect("Failed to create fixtures directory");
    std::fs::write(fixtures_dir.join("existing.txt"), "ok").expect("Failed to create fixture");

    project.acton().test().run().success().assert_passed(1);
}
