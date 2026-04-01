use crate::debugging::support::assertions::{DebugTestOutput, DebugTestOutputExt};
use crate::debugging::support::debug::DebugBuilder;

#[test]
fn test_repeat_10_step_over() -> anyhow::Result<()> {
    let code = r"
fun main() {
    var a = 0;
    repeat(10) {
        a += 1;
    }
    return a + 1;
}
";

    let session = DebugBuilder::new("debug-callback").code(code).build();

    let mut client = session.start();

    let result = client.execute(|executor| {
        executor.step_over_times(5)?;
        Ok(())
    })?;

    // TODO: for such simple loop we step only by `a += 1` line and step-over logic skip
    //       same line steps and we actually skips whole loop
    let debug_output = DebugTestOutput::new(result);
    debug_output
        .assert_trace_snapshot_matches("debugging/snapshots/repeat/with_10_step_over.trace.txt");

    Ok(())
}

#[test]
fn test_repeat_2_step_in() -> anyhow::Result<()> {
    let code = r"
fun main() {
    var a = 0;
    repeat(2) {
        a += 1;
    }
    return a + 1;
}
";

    let session = DebugBuilder::new("debug-callback").code(code).build();

    let mut client = session.start();

    let result = client.execute(|executor| {
        executor.step_in_times(20)?;
        Ok(())
    })?;

    let debug_output = DebugTestOutput::new(result);
    debug_output
        .assert_trace_snapshot_matches("debugging/snapshots/repeat/with_2_step_in.trace.txt");

    Ok(())
}

#[test]
fn test_repeat_2_step_over_with_complex_body() -> anyhow::Result<()> {
    let code = r"
fun main() {
    var a = 0;
    repeat(2) {
        if (a == 0) {
            a += 10;
        } else {
            a += 20;
        }
    }
    return a + 1;
}
";

    let session = DebugBuilder::new("debug-callback").code(code).build();

    let mut client = session.start();

    let result = client.execute(|executor| {
        executor.step_over_times(3)?;
        executor.step_in_times(9)?;
        Ok(())
    })?;

    let debug_output = DebugTestOutput::new(result);
    debug_output.assert_trace_snapshot_matches(
        "debugging/snapshots/repeat/with_2_step_over_complex_body.trace.txt",
    );

    Ok(())
}
