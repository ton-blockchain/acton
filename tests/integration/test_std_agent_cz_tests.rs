//! Reserved for agent-cz.
//! Prefix: cz_stdlib_
//! Ownership: this file and tests/integration/snapshots/test_std_agent_cz/**
//! Agent-owned tests for LibRef.fromTuple branch decoding.

use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const CZ_OUT_ACTIONS_IMPORTS: &str = r#"
import "../../lib/testing/assert"
import "../../lib/testing/expect"
import "../../lib/types/out_actions"
"#;

fn run_cz_stdlib_failure(project_name: &str, test_body: &str, snapshot_path: &str) {
    let source = format!("{CZ_OUT_ACTIONS_IMPORTS}\n{test_body}\n");
    ProjectBuilder::new(project_name)
        .test_file("libref_from_tuple", &source)
        .build()
        .acton()
        .test()
        .run()
        .failure()
        .assert_failed(1)
        .assert_snapshot_matches(snapshot_path);
}

#[test]
fn cz_stdlib_libref_from_tuple_decodes_remove_hash_branch() {
    run_cz_stdlib_failure(
        "cz-stdlib-libref-from-tuple-remove-hash-branch",
        r#"
get fun `test-cz-libref-from-tuple-remove-hash-branch`() {
    val expectedHash = beginCell()
        .storeUint(0xC0DE, 16)
        .storeUint(0x77, 8)
        .endCell()
        .hash();
    var raw = createEmptyTuple();
    raw.push(0);
    raw.push(expectedHash);

    // BUG: LibRef.fromTuple should decode tuple tag 0 into LibRefHash, but throws "not a tuple of valid size" (exit_code=7).
    val decoded = LibRef.fromTuple(raw);
    if (decoded is LibRefHash) {
        expect(decoded.libHash).toEqual(expectedHash);
    } else {
        Assert.fail("expected LibRefHash for tuple tag 0");
    }
}
"#,
        "integration/snapshots/test_std_agent_cz/cz_stdlib_libref_from_tuple_decodes_remove_hash_branch.stdout.txt",
    );
}

#[test]
fn cz_stdlib_libref_from_tuple_decodes_publish_cell_branch() {
    run_cz_stdlib_failure(
        "cz-stdlib-libref-from-tuple-publish-cell-branch",
        r#"
get fun `test-cz-libref-from-tuple-publish-cell-branch`() {
    val expectedCell = beginCell()
        .storeUint(0xBEEF, 16)
        .storeUint(0xCAFE, 16)
        .endCell();
    var raw = createEmptyTuple();
    raw.push(1);
    raw.push(expectedCell);

    // BUG: LibRef.fromTuple should decode tuple tag 1 into LibRefRef, but throws "not a tuple of valid size" (exit_code=7).
    val decoded = LibRef.fromTuple(raw);
    if (decoded is LibRefRef) {
        expect(decoded.library).toEqual(expectedCell);
    } else {
        Assert.fail("expected LibRefRef for tuple tag 1");
    }
}
"#,
        "integration/snapshots/test_std_agent_cz/cz_stdlib_libref_from_tuple_decodes_publish_cell_branch.stdout.txt",
    );
}
