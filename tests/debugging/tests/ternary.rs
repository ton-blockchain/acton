use crate::debugging::support::assertions::{DebugTestOutput, DebugTestOutputExt};
use crate::debugging::support::debug::DebugBuilder;

#[test]
fn test_single_line_ternary_over_nullable_int_step_over() -> anyhow::Result<()> {
    let code = r#"
fun main(foo: int?) {
    val a = foo != null ? foo : 100;
    return a + 1;
}
"#;

    let session = DebugBuilder::new("debug-callback")
        .code(code)
        .accept_int(200)
        .build();

    let mut client = session.start();

    let result = client.execute(|executor| {
        executor.step_over()?;
        Ok(())
    })?;

    let debug_output = DebugTestOutput::new(result);
    debug_output.assert_trace_snapshot_matches(
        "debugging/snapshots/ternary/single_line_over_nullable_int_step_over.trace.txt",
    );

    Ok(())
}

#[test]
fn test_single_line_ternary_over_nullable_int_step_in() -> anyhow::Result<()> {
    let code = r#"
fun main(foo: int?) {
    val a = foo != null ? foo : 100;
    return a + 1;
}
"#;

    let session = DebugBuilder::new("debug-callback")
        .code(code)
        .accept_int(200)
        .build();

    let mut client = session.start();

    let result = client.execute(|executor| {
        executor.step_in()?;
        executor.step_in()?;
        executor.step_in()?;
        executor.step_in()?;
        executor.step_in()?;
        Ok(())
    })?;

    let debug_output = DebugTestOutput::new(result);
    debug_output.assert_trace_snapshot_matches(
        "debugging/snapshots/ternary/single_line_over_nullable_int_step_in.trace.txt",
    );

    Ok(())
}

#[test]
fn test_multi_line_ternary_over_nullable_int_step_over() -> anyhow::Result<()> {
    let code = r#"
fun main(foo: int?) {
    val a = foo != null
        ? foo
        : 100;
    return a + 1;
}
"#;

    let session = DebugBuilder::new("debug-callback")
        .code(code)
        .accept_int(200)
        .build();

    let mut client = session.start();

    // TODO: ßDue to optimization to CONDSEL we likely cannot jump to correct branch
    let result = client.execute(|executor| {
        executor.step_over()?;
        executor.step_over()?;
        Ok(())
    })?;

    let debug_output = DebugTestOutput::new(result);
    debug_output.assert_trace_snapshot_matches(
        "debugging/snapshots/ternary/multi_line_over_nullable_int_step_over.trace.txt",
    );

    Ok(())
}

#[test]
fn test_multi_line_ternary_with_complex_condition_step_over_true() -> anyhow::Result<()> {
    let code = r#"
fun main(foo: int?) {
    val a = foo != null && foo == 100
        ? foo + 1
        : 100;
    return a + 1;
}
"#;

    let session = DebugBuilder::new("debug-callback")
        .code(code)
        .accept_int(100)
        .build();

    let mut client = session.start();

    let result = client.execute(|executor| {
        executor.step_over()?;
        executor.step_over()?;
        executor.step_over()?;
        executor.step_over()?;
        executor.step_over()?;
        executor.step_over()?;
        Ok(())
    })?;

    let debug_output = DebugTestOutput::new(result);
    debug_output.assert_trace_snapshot_matches(
        "debugging/snapshots/ternary/multi_line_with_complex_condition_step_over_true.trace.txt",
    );

    Ok(())
}

#[test]
fn test_multi_line_ternary_with_complex_condition_step_over_false() -> anyhow::Result<()> {
    let code = r#"
fun main(foo: int?) {
    val a = foo != null && foo == 100
        ? foo + 1
        : 100;
    return a + 1;
}
"#;

    let session = DebugBuilder::new("debug-callback")
        .code(code)
        .accept_int(200)
        .build();

    let mut client = session.start();

    let result = client.execute(|executor| {
        executor.step_over()?;
        executor.step_over()?;
        executor.step_over()?;
        executor.step_over()?;
        executor.step_over()?;
        executor.step_over()?;
        Ok(())
    })?;

    let debug_output = DebugTestOutput::new(result);
    debug_output.assert_trace_snapshot_matches(
        "debugging/snapshots/ternary/multi_line_with_complex_condition_step_over_false.trace.txt",
    );

    Ok(())
}

#[test]
fn test_multi_line_ternary_with_complex_condition_step_in_true() -> anyhow::Result<()> {
    let code = r#"
fun main(foo: int?) {
    val a = foo != null && foo == 100
        ? foo + 1
        : 100;
    return a + 1;
}
"#;

    let session = DebugBuilder::new("debug-callback")
        .code(code)
        .accept_int(100)
        .build();

    let mut client = session.start();

    let result = client.execute(|executor| {
        for _ in 0..19 {
            executor.step_in()?;
        }
        Ok(())
    })?;

    let debug_output = DebugTestOutput::new(result);
    debug_output.assert_trace_snapshot_matches(
        "debugging/snapshots/ternary/multi_line_with_complex_condition_step_in_true.trace.txt",
    );

    Ok(())
}
