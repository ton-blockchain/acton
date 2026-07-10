use super::support::CompletionTest;
use expect_test::expect;

#[test]
fn completes_all_entry_point_templates() {
    // Top-level completion offers every supported contract entry point.
    CompletionTest::new(
        "
            fun onInternalMessage(in: InMessage) {}
            <caret>
        ",
    )
        .labels(&[
            "onInternalMessage",
            "onExternalMessage",
            "onBouncedMessage",
            "onTickTock",
        ])
        .check(expect![[r#"
            label              kind     detail  edit     text
            onBouncedMessage   Snippet          1:0-1:0  fun onBouncedMessage(in: InMessageBounced) {\n    $0\n}
            onExternalMessage  Snippet          1:0-1:0  fun onExternalMessage(inMsg: slice) {\n    $0\n}
            onInternalMessage  Snippet          1:0-1:0  fun onInternalMessage(in: InMessage) {\n    $0\n}
            onTickTock         Snippet          1:0-1:0  fun onTickTock(isTock: bool) {\n    $0\n}"#]]);
}

#[test]
fn applies_each_entry_point_template() {
    // Internal-message completion inserts the canonical InMessage signature.
    CompletionTest::new("onInt<caret>").check_applied(
        "onInternalMessage",
        expect![[r#"
            fun onInternalMessage(in: InMessage) {
                <caret>
            }"#]],
    );

    // Bounced-message completion inserts the InMessageBounced signature.
    CompletionTest::new("onBou<caret>").check_applied(
        "onBouncedMessage",
        expect![[r#"
            fun onBouncedMessage(in: InMessageBounced) {
                <caret>
            }"#]],
    );

    // External-message completion inserts the slice signature.
    CompletionTest::new("onExt<caret>").check_applied(
        "onExternalMessage",
        expect![[r#"
            fun onExternalMessage(inMsg: slice) {
                <caret>
            }"#]],
    );

    // Tick-tock completion inserts its boolean discriminator parameter.
    CompletionTest::new("onTick<caret>").check_applied(
        "onTickTock",
        expect![[r#"
            fun onTickTock(isTock: bool) {
                <caret>
            }"#]],
    );
}
