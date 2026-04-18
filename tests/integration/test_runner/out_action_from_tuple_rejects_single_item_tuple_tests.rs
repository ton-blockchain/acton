use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

#[test]
fn parse_out_actions_rejects_node_without_prev_ref() {
    ProjectBuilder::new("stdlib-parse-out-actions-rejects-node-without-prev-ref")
        .test_file("out_actions_malformed", r#"
            import "../../lib/types/out_actions"
            import "../../lib/emulation/testing"

            get fun `test parse out actions rejects node without prev ref`() {
                val malformedNode = beginCell()
                    .storeUint(0x0ec3c86d, 32)
                    .storeUint(0, 8)
                    .endCell();

                __acton_impl_parseOutActions(malformedNode);
            }
            "#)
        .build()
        .acton()
        .test()
        .run()
        .failure()
        .assert_failed(1)
        .assert_contains("Malformed out action list node")
        .assert_snapshot_matches("integration/snapshots/test-runner/out_action_from_tuple_rejects_single_item_tuple/parse_out_actions_rejects_node_without_prev_ref.stdout.txt");
}
