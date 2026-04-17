use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;
use tycho_types::cell::CellBuilder;

const IMPORTS: &str = r#"
import "../../lib/emulation/network"
import "../../lib/testing/expect"
import "../../lib/vm/vm"
"#;

const DUMMY_CONTRACT: &str = r"
fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
";

fn registered_library_hash_hex() -> String {
    let mut builder = CellBuilder::new();
    builder
        .store_u32(0xcafe_babe)
        .expect("must build library cell for test");
    let cell = builder
        .build()
        .expect("must finalize library cell for test");
    hex::encode(cell.repr_hash().as_array())
}

#[test]
fn load_library_prefers_world_state_library_before_network() {
    let library_hash = registered_library_hash_hex();
    let source = format!(
        r#"
{IMPORTS}

get fun `test load library prefers world state library before network`() {{
    val libraryCode = beginCell().storeUint(0xcafe_babe, 32).endCell();
    vm.registerLibrary(libraryCode);

    val loaded = net.loadLibrary("{library_hash}");
    expect(loaded).toBeNotNull();

    if (loaded != null) {{
        expect(loaded).toEqual(libraryCode);
    }}
}}
"#
    );

    ProjectBuilder::new("stdlib-load-library-prefers-world-state-before-network")
        .contract("dummy", DUMMY_CONTRACT)
        .test_file("load_library_prefers_world_state", &source)
        .build()
        .acton()
        .test()
        .fork_net("custom:bm-missing-net")
        .run()
        .success()
        .assert_passed(1);
}
