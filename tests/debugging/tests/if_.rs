use crate::debugging::support::assertions::{DebugTestOutput, DebugTestOutputExt};
use crate::debugging::support::debug::DebugBuilder;
use tvm_ffi::stack::{Tuple, TupleItem};

#[test]
fn test_if_over_numbers_with_first_matching() -> anyhow::Result<()> {
    let code = r"
global foo: int;

fun main() {
    foo = 100;
    if (foo == 100) {
        return 10;
    }

    return 0;
}
";

    let session = DebugBuilder::new("debug-callback").code(code).build();

    let mut client = session.start();

    let result = client.execute(|executor| {
        executor.step_over_times(4)?;
        Ok(())
    })?;

    let debug_output = DebugTestOutput::new(result);
    debug_output.assert_trace_snapshot_matches(
        "debugging/snapshots/if/over_numbers_with_first_matching.trace.txt",
    );

    Ok(())
}

#[test]
fn test_if_over_numbers_with_second_matching() -> anyhow::Result<()> {
    let code = r"
global foo: int;

fun main() {
    foo = 200;
    if (foo == 100) {
        return 10;
    } else if (foo == 200) {
        return 20
    }

    return 0;
}
";

    let session = DebugBuilder::new("debug-callback").code(code).build();

    let mut client = session.start();

    let result = client.execute(|executor| {
        executor.step_over_times(4)?;
        Ok(())
    })?;

    let debug_output = DebugTestOutput::new(result);
    debug_output.assert_trace_snapshot_matches(
        "debugging/snapshots/if/over_numbers_with_second_matching.trace.txt",
    );

    Ok(())
}

#[test]
fn test_if_over_numbers_with_else_matching() -> anyhow::Result<()> {
    let code = r"
global foo: int;

fun main() {
    foo = 300;
    if (foo == 100) {
        return 10;
    } else if (foo == 200) {
        return 20
    } else {
        return 30
    }

    return 0;
}
";

    let session = DebugBuilder::new("debug-callback").code(code).build();

    let mut client = session.start();

    let result = client.execute(|executor| {
        executor.step_over_times(9)?;
        Ok(())
    })?;

    let debug_output = DebugTestOutput::new(result);
    debug_output.assert_trace_snapshot_matches(
        "debugging/snapshots/if/over_numbers_with_else_matching.trace.txt",
    );

    Ok(())
}

#[test]
fn test_if_return_with_nullable_true_step_over() -> anyhow::Result<()> {
    let code = r#"
fun main(foo: int?) {
    if (foo == null) {
        return
    }

    debug.print("aaa");
}
"#;

    let session = DebugBuilder::new("debug-callback")
        .code(code)
        .stack(Tuple(vec![TupleItem::Null]))
        .build();

    let mut client = session.start();

    let result = client.execute(|executor| {
        executor.step_over()?;
        Ok(())
    })?;

    let debug_output = DebugTestOutput::new(result);
    debug_output.assert_trace_snapshot_matches(
        "debugging/snapshots/if/return_with_nullable_true_step_over.trace.txt",
    );

    Ok(())
}

#[test]
fn test_if_return_with_nullable_false_step_over() -> anyhow::Result<()> {
    let code = r#"
fun main(foo: int?) {
    if (foo == null) {
        return
    }

    debug.print("aaa");
}
"#;

    let session = DebugBuilder::new("debug-callback")
        .code(code)
        .accept_int(100)
        .build();

    let mut client = session.start();

    let result = client.execute(|executor| {
        executor.step_over()?;
        Ok(())
    })?;

    let debug_output = DebugTestOutput::new(result);
    debug_output.assert_trace_snapshot_matches(
        "debugging/snapshots/if/return_with_nullable_false_step_over.trace.txt",
    );

    Ok(())
}

#[test]
fn test_if_return_with_nullable_true() -> anyhow::Result<()> {
    let code = r#"
fun main(foo: int?) {
    if (foo == null) {
        return
    }

    debug.print("aaa");
}
"#;

    let session = DebugBuilder::new("debug-callback")
        .code(code)
        .stack(Tuple(vec![TupleItem::Null]))
        .build();

    let mut client = session.start();

    let result = client.execute(|executor| {
        executor.step_in()?;
        Ok(())
    })?;

    let debug_output = DebugTestOutput::new(result);
    debug_output.assert_trace_snapshot_matches(
        "debugging/snapshots/if/return_with_nullable_true.trace.txt",
    );

    Ok(())
}

#[test]
fn test_if_return_with_nullable_false() -> anyhow::Result<()> {
    let code = r#"
fun main(foo: int?) {
    if (foo == null) {
        return
    }

    debug.print("aaa");
}
"#;

    let session = DebugBuilder::new("debug-callback")
        .code(code)
        .accept_int(100)
        .build();

    let mut client = session.start();

    let result = client.execute(|executor| {
        executor.step_in()?;
        executor.step_in()?;
        Ok(())
    })?;

    let debug_output = DebugTestOutput::new(result);
    debug_output.assert_trace_snapshot_matches(
        "debugging/snapshots/if/return_with_nullable_false.trace.txt",
    );

    Ok(())
}
