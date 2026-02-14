use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const SIMPLE_CONTRACT: &str = r"
fun onInternalMessage(in: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
";

#[test]
fn test_check_success() {
    let project = ProjectBuilder::new("check-success")
        .contract("simple", SIMPLE_CONTRACT)
        .build();

    project.acton().init().run().success();

    project
        .acton()
        .check()
        .run()
        .success()
        .assert_snapshot_matches("integration/snapshots/check/test_check_success.stdout.txt");
}
