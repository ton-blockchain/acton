use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;
use serde_json::json;
use std::fs;
use tycho_types::boc::{Boc, ser::BocHeader};
use tycho_types::cell::{Cell, CellBuilder};

const BOC_IMPORTS: &str = r#"
import "../../lib/boc"
import "../../lib/fs"
import "../../lib/testing/expect"
"#;

fn build_boc_project(project_name: &str, test_code: &str) -> crate::support::project::Project {
    let full_code = format!("{BOC_IMPORTS}\n{test_code}\n");
    ProjectBuilder::new(project_name)
        .test_file("boc", &full_code)
        .build()
}

fn byte_cell(value: u8) -> Cell {
    let mut builder = CellBuilder::new();
    builder.store_u8(value).expect("byte must fit into a cell");
    builder.build().expect("byte cell must build")
}

#[test]
fn boc_encode_writes_raw_cell_and_slice_bytes_with_optional_crc() {
    let project = build_boc_project(
        "w-stdlib-boc-encode-raw-bytes",
        r#"
struct BocPayload {
    value: uint16
}

get fun `test boc encode cell typed cell and crc`() {
    val child = beginCell().storeUint(0xAB, 8).endCell();
    val root = beginCell()
        .storeUint(0x1234, 16)
        .storeRef(child)
        .endCell();

    val plain = boc.encode(root);
    expect(fs.writeBytes("plain.boc", plain)).toBeTrue();
    val decodedPlain = boc.decode(plain);
    expect(decodedPlain).toBeNotNull();
    expect(decodedPlain!.hash()).toEqual(root.hash());

    val withCrc = boc.encode(root, { crc32: true });
    expect(fs.writeBytes("crc.boc", withCrc)).toBeTrue();
    val decodedCrc = boc.decode(withCrc);
    expect(decodedCrc).toBeNotNull();
    expect(decodedCrc!.hash()).toEqual(root.hash());

    val typed = BocPayload { value: 0xCAFE }.toCell();
    val decodedTyped = boc.decode(boc.encode(typed));
    expect(decodedTyped).toBeNotNull();
    expect(decodedTyped!.hash()).toEqual(typed.hash());
}

get fun `test boc encode uses unread slice remainder`() {
    val skippedRef = beginCell().storeUint(0x11, 8).endCell();
    val keptRef = beginCell().storeUint(0x22, 8).endCell();
    var source = beginCell()
        .storeUint(0xAA, 8)
        .storeUint(0xBEEF, 16)
        .storeRef(skippedRef)
        .storeRef(keptRef)
        .endCell()
        .beginParse();

    expect(source.loadUint(8)).toEqual(0xAA);
    expect(source.loadRef().hash()).toEqual(skippedRef.hash());

    val encoded = boc.encode(source);
    expect(fs.writeBytes("slice.boc", encoded)).toBeTrue();
    val decoded = boc.decode(encoded);
    expect(decoded).toBeNotNull();

    var remainder = decoded!.beginParse();
    expect(remainder.remainingBitsCount()).toEqual(16);
    expect(remainder.remainingRefsCount()).toEqual(1);
    expect(remainder.loadUint(16)).toEqual(0xBEEF);
    expect(remainder.loadRef().hash()).toEqual(keptRef.hash());
}
"#,
    );

    project
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(2)
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/boc_encode_decode/boc_encode_writes_raw_cell_and_slice_bytes_with_optional_crc.stdout.txt",
        );

    let plain = fs::read(project.path().join("plain.boc")).expect("plain BoC must be written");
    let with_crc = fs::read(project.path().join("crc.boc")).expect("CRC BoC must be written");
    let slice = fs::read(project.path().join("slice.boc")).expect("slice BoC must be written");

    let child = byte_cell(0xAB);
    let mut root_builder = CellBuilder::new();
    root_builder.store_u16(0x1234).expect("root bits must fit");
    root_builder
        .store_reference(child)
        .expect("root reference must fit");
    let expected_root = root_builder.build().expect("root cell must build");

    let kept_ref = byte_cell(0x22);
    let mut remainder_builder = CellBuilder::new();
    remainder_builder
        .store_u16(0xBEEF)
        .expect("remainder bits must fit");
    remainder_builder
        .store_reference(kept_ref)
        .expect("remainder reference must fit");
    let expected_remainder = remainder_builder
        .build()
        .expect("remainder cell must build");

    let summary = json!({
        "plain": {
            "hex": hex::encode(&plain),
            "matches_tycho_encode": plain == Boc::encode(&expected_root),
            "decoded_hash_matches": Boc::decode(&plain)
                .is_ok_and(|cell| cell.repr_hash() == expected_root.repr_hash()),
        },
        "crc": {
            "hex": hex::encode(&with_crc),
            "flag_set": with_crc.get(4).is_some_and(|flags| flags & 0x40 != 0),
            "four_bytes_longer": with_crc.len() == plain.len() + 4,
            "decoded_hash_matches": Boc::decode(&with_crc)
                .is_ok_and(|cell| cell.repr_hash() == expected_root.repr_hash()),
        },
        "slice_remainder": {
            "hex": hex::encode(&slice),
            "decoded_hash_matches": Boc::decode(&slice)
                .is_ok_and(|cell| cell.repr_hash() == expected_remainder.repr_hash()),
        },
    });
    snapbox::assert_data_eq!(
        format!(
            "{}\n",
            serde_json::to_string_pretty(&summary).expect("summary must serialize")
        ),
        snapbox::file!(
            "../snapshots/test-runner/boc_encode_decode/boc_encode_writes_raw_cell_and_slice_bytes_with_optional_crc.summary.json"
        )
    );
}

#[test]
fn boc_decode_accepts_indexed_boc_and_returns_null_for_invalid_inputs() {
    let project = build_boc_project(
        "w-stdlib-boc-decode-invalid-inputs",
        r#"
get fun `test boc decode accepts indexed and rejects invalid inputs`() {
    val indexed = boc.decode(fs.readBytes("fixtures/indexed-empty.boc")!);
    expect(indexed).toBeNotNull();
    expect(indexed!.beginParse().remainingBitsCount()).toEqual(0);
    expect(indexed!.beginParse().remainingRefsCount()).toEqual(0);

    expect(boc.decode(fs.readBytes("fixtures/not-a-boc.bin")!)).toBeNull();
    expect(boc.decode(fs.readBytes("fixtures/bad-crc.boc")!)).toBeNull();
    expect(boc.decode(fs.readBytes("fixtures/multi-root.boc")!)).toBeNull();

    val nonByteAligned = beginCell().storeUint(1, 1).toSlice();
    expect(boc.decode(nonByteAligned)).toBeNull();
}
"#,
    );

    let fixtures = project.path().join("fixtures");
    fs::create_dir_all(&fixtures).expect("fixtures directory must be created");

    // Generic single-root BoC for an empty cell with has_idx=1 and one end-offset entry.
    fs::write(
        fixtures.join("indexed-empty.boc"),
        hex::decode("b5ee9c7281010101000200020000").expect("indexed fixture hex must decode"),
    )
    .expect("indexed fixture must be written");
    fs::write(fixtures.join("not-a-boc.bin"), b"not a boc")
        .expect("invalid fixture must be written");

    let crc_cell = byte_cell(0xCC);
    let mut bad_crc = Vec::new();
    BocHeader::<std::collections::hash_map::RandomState>::with_root(crc_cell.as_ref())
        .with_crc(true)
        .encode(&mut bad_crc);
    *bad_crc.last_mut().expect("CRC BoC must not be empty") ^= 0xFF;
    fs::write(fixtures.join("bad-crc.boc"), bad_crc).expect("bad CRC fixture must be written");

    let first_root = byte_cell(0x01);
    let second_root = byte_cell(0x02);
    let mut multi_root_header =
        BocHeader::<std::collections::hash_map::RandomState>::with_root(first_root.as_ref());
    multi_root_header.add_root(second_root.as_ref());
    let mut multi_root = Vec::new();
    multi_root_header.encode(&mut multi_root);
    fs::write(fixtures.join("multi-root.boc"), multi_root)
        .expect("multi-root fixture must be written");

    project
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/boc_encode_decode/boc_decode_accepts_indexed_boc_and_returns_null_for_invalid_inputs.stdout.txt",
        );
}
