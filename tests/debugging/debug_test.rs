use crate::debugging::support::assertions::{DebugTestOutput, DebugTestOutputExt};
use crate::debugging::support::debug::DebugBuilder;

#[test]
fn test_simple_step_by_step_execution() -> anyhow::Result<()> {
    let code = r"
global foo: int;

fun main() {
    foo = 100;
    return foo;
}
";

    let session = DebugBuilder::new("debug-callback").code(code).build();

    let mut client = session.start();

    let result = client.execute(|executor| {
        executor.step_in_times(5)?;
        Ok(())
    })?;

    let debug_output = DebugTestOutput::new(result);
    debug_output.assert_trace_steps(4);
    debug_output.assert_trace_snapshot_matches(
        "debugging/snapshots/test_simple_step_by_step_execution.trace.txt",
    );

    Ok(())
}

#[test]
fn test_simple_step_by_step_execution_with_step_over() -> anyhow::Result<()> {
    let code = r"
global foo: int;

fun main() {
    foo = 100;
    foo = 200;
    return foo;
}
";

    let session = DebugBuilder::new("debug-callback").code(code).build();

    let mut client = session.start();

    let result = client.execute(|executor| {
        executor.step_over_times(2)?;
        Ok(())
    })?;

    let debug_output = DebugTestOutput::new(result);
    debug_output.assert_trace_steps(3);
    debug_output.assert_trace_snapshot_matches(
        "debugging/snapshots/test_simple_step_by_step_execution_with_step_over.trace.txt",
    );

    Ok(())
}

#[test]
fn test_can_continue() -> anyhow::Result<()> {
    let code = r"
global foo: int;

fun main() {
    foo = 100;
    return foo;
}
";

    let session = DebugBuilder::new("debug-callback").code(code).build();

    let mut client = session.start();

    let result = client.execute(|executor| {
        executor.continue_execution()?;
        Ok(())
    })?;

    let debug_output = DebugTestOutput::new(result);

    // Continue doesn't add any steps, so we expect zero + 1 for initial state
    debug_output.assert_trace_steps(1);

    Ok(())
}

#[test]
fn test_simple_debug_with_stack_argument() -> anyhow::Result<()> {
    let code = r"
global result: int;

fun main(arg: int) {
    result = arg * 2;
    return result;
}
";

    let session = DebugBuilder::new("debug-with-stack")
        .code(code)
        .accept_int(21)
        .build();

    let mut client = session.start();

    let result = client.execute(|executor| {
        executor.step_in_times(6)?;
        Ok(())
    })?;

    let debug_output = DebugTestOutput::new(result);
    debug_output.assert_trace_snapshot_matches(
        "debugging/snapshots/test_simple_debug_with_stack_argument.trace.txt",
    );

    Ok(())
}
