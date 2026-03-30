use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const EO_TRANSACTION_IMPORTS: &str = r#"
import "../../lib/testing/expect"
import "../../lib/types/transaction"

struct EoVarUint7Box {
    value: VarUint7
}

struct EoVarUint3Box {
    value: VarUint3
}

fun canonicalVarUint7Box(value: int): Cell<EoVarUint7Box> {
    var b = beginCell();
    if (value == 0) {
        b.storeUint(0, 3);
    } else if (value <= 255) {
        b.storeUint(1, 3);
        b.storeUint(value, 8);
    } else {
        b.storeUint(2, 3);
        b.storeUint(value, 16);
    }
    return b.endCell() as Cell<EoVarUint7Box>;
}

fun canonicalVarUint3Box(value: int): Cell<EoVarUint3Box> {
    var b = beginCell();
    if (value == 0) {
        b.storeUint(0, 2);
    } else if (value <= 255) {
        b.storeUint(1, 2);
        b.storeUint(value, 8);
    } else if (value <= 65535) {
        b.storeUint(2, 2);
        b.storeUint(value, 16);
    } else {
        b.storeUint(3, 2);
        b.storeUint(value, 24);
    }
    return b.endCell() as Cell<EoVarUint3Box>;
}
"#;

fn run_success_case(project_name: &str, test_body: &str, snapshot_path: &str) {
    let source = format!("{EO_TRANSACTION_IMPORTS}\n{test_body}\n");
    ProjectBuilder::new(project_name)
        .test_file("eo_transaction_varuint", &source)
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(snapshot_path);
}

#[test]
fn transaction_varuint_unpack_from_slice_supports_canonical_boundary_values() {
    run_success_case(
        "eo-stdlib-transaction-varuint-unpack-canonical-boundaries",
        r"
get fun `test-eo-varuint-unpack-canonical-boundaries`() {
    expect(canonicalVarUint7Box(0).load().value).toEqual(0);
    expect(canonicalVarUint7Box(255).load().value).toEqual(255);
    expect(canonicalVarUint7Box(256).load().value).toEqual(256);
    expect(canonicalVarUint7Box(65535).load().value).toEqual(65535);

    expect(canonicalVarUint3Box(0).load().value).toEqual(0);
    expect(canonicalVarUint3Box(255).load().value).toEqual(255);
    expect(canonicalVarUint3Box(256).load().value).toEqual(256);
    expect(canonicalVarUint3Box(65535).load().value).toEqual(65535);
    expect(canonicalVarUint3Box(16777215).load().value).toEqual(16777215);
}
",
        "integration/snapshots/test-runner/transaction_varuint_unpack_from_slice_supports_canonical_boundary_values/transaction_varuint_unpack_from_slice_supports_canonical_boundary_values.stdout.txt",
    );
}

#[test]
fn transaction_varuint7_pack_to_builder_roundtrip_boundary_bug() {
    run_success_case(
        "eo-stdlib-transaction-varuint7-pack-roundtrip-boundary-bug",
        r"
get fun `test-eo-varuint7-pack-roundtrip-boundary-bug`() {
    val source = EoVarUint7Box { value: 0 };

    var directBuilder = beginCell();
    source.value.packToBuilder(mutate directBuilder);
    val directCell = directBuilder.endCell();
    var directSlice = directCell.beginParse();

    val directDecoded = VarUint7.unpackFromSlice(mutate directSlice);
    expect(directDecoded).toEqual(source.value);

    val decoded = EoVarUint7Box.fromCell(source.toCell());
    expect(decoded.value).toEqual(source.value);
}
",
        "integration/snapshots/test-runner/transaction_varuint_unpack_from_slice_supports_canonical_boundary_values/transaction_varuint7_pack_to_builder_roundtrip_boundary_bug.stdout.txt",
    );
}

#[test]
fn transaction_varuint3_pack_to_builder_roundtrip_boundary_bug() {
    run_success_case(
        "eo-stdlib-transaction-varuint3-pack-roundtrip-boundary-bug",
        r"
get fun `test-eo-varuint3-pack-roundtrip-boundary-bug`() {
    val source = EoVarUint3Box { value: 255 };

    var directBuilder = beginCell();
    source.value.packToBuilder(mutate directBuilder);
    val directCell = directBuilder.endCell();
    var directSlice = directCell.beginParse();

    val directDecoded = VarUint3.unpackFromSlice(mutate directSlice);
    expect(directDecoded).toEqual(source.value);

    val decoded = EoVarUint3Box.fromCell(source.toCell());
    expect(decoded.value).toEqual(source.value);
}
",
        "integration/snapshots/test-runner/transaction_varuint_unpack_from_slice_supports_canonical_boundary_values/transaction_varuint3_pack_to_builder_roundtrip_boundary_bug.stdout.txt",
    );
}
