use crate::debugging::support::assertions::{DebugTestOutput, DebugTestOutputExt};
use crate::debugging::support::debug::DebugBuilder;

#[test]
fn test_simple_step_by_step_execution() -> anyhow::Result<()> {
    let code = r#"
global foo: int;

fun main() {
    foo = 100;
    return foo;
}
"#;

    let session = DebugBuilder::new("debug-callback").code(code).build();

    let mut client = session.start();

    let result = client.execute(|executor| {
        executor.step_in()?;
        executor.step_in()?;
        executor.step_in()?;
        executor.step_in()?;
        Ok(())
    })?;

    let debug_output = DebugTestOutput::new(result);
    debug_output.assert_trace_steps(5);
    debug_output.assert_trace_snapshot_matches(
        "debugging/snapshots/test_simple_step_by_step_execution.trace.txt",
    );

    Ok(())
}

#[test]
fn test_simple_step_by_step_execution_with_step_over() -> anyhow::Result<()> {
    let code = r#"
global foo: int;

fun main() {
    foo = 100;
    foo = 200;
    return foo;
}
"#;

    let session = DebugBuilder::new("debug-callback").code(code).build();

    let mut client = session.start();

    let result = client.execute(|executor| {
        executor.step_over()?;
        executor.step_over()?;
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
    let code = r#"
global foo: int;

fun main() {
    foo = 100;
    return foo;
}
"#;

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
fn test_match_over_numbers_with_first_matching() -> anyhow::Result<()> {
    let code = r#"
global foo: int;

fun main() {
    foo = 100;
    match (foo) {
        100 => {
            return 10
        }
        200 => {
            return 20
        }
    }

    return 0;
}
"#;

    let session = DebugBuilder::new("debug-callback").code(code).build();

    let mut client = session.start();

    let result = client.execute(|executor| {
        executor.step_over()?;
        executor.step_over()?;
        executor.step_over()?;
        Ok(())
    })?;

    let debug_output = DebugTestOutput::new(result);
    debug_output.assert_trace_snapshot_matches(
        "debugging/snapshots/test_match_over_numbers_with_first_matching.trace.txt",
    );

    Ok(())
}

#[test]
fn test_match_over_numbers_with_second_matching() -> anyhow::Result<()> {
    let code = r#"
global foo: int;

fun main() {
    foo = 200;
    match (foo) {
        100 => {
            return 10
        }
        200 => {
            return 20
        }
    }

    return 0;
}
"#;

    let session = DebugBuilder::new("debug-callback").code(code).build();

    let mut client = session.start();

    // TODO
    let result = client.execute(|executor| {
        executor.step_over()?;
        executor.step_over()?;
        executor.step_over()?;
        executor.step_over()?;
        executor.step_over()?;
        Ok(())
    })?;

    let debug_output = DebugTestOutput::new(result);
    debug_output.assert_trace_snapshot_matches(
        "debugging/snapshots/test_match_over_numbers_with_second_matching.trace.txt",
    );

    Ok(())
}

#[test]
fn test_match_over_numbers_with_else_matching() -> anyhow::Result<()> {
    let code = r#"
global foo: int;

fun main() {
    foo = 300;
    match (foo) {
        100 => {
            return 10
        }
        200 => {
            return 20
        }
        else => {
            return 30
        }
    }

    return 0;
}
"#;

    let session = DebugBuilder::new("debug-callback").code(code).build();

    let mut client = session.start();

    // TODO
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
        "debugging/snapshots/test_match_over_numbers_with_else_matching.trace.txt",
    );

    Ok(())
}

#[test]
fn test_if_over_numbers_with_first_matching() -> anyhow::Result<()> {
    let code = r#"
global foo: int;

fun main() {
    foo = 100;
    if (foo == 100) {
        return 10;
    }

    return 0;
}
"#;

    let session = DebugBuilder::new("debug-callback").code(code).build();

    let mut client = session.start();

    let result = client.execute(|executor| {
        executor.step_over()?;
        executor.step_over()?;
        Ok(())
    })?;

    let debug_output = DebugTestOutput::new(result);
    debug_output.assert_trace_snapshot_matches(
        "debugging/snapshots/test_if_over_numbers_with_first_matching.trace.txt",
    );

    Ok(())
}

#[test]
fn test_if_over_numbers_with_second_matching() -> anyhow::Result<()> {
    let code = r#"
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
"#;

    let session = DebugBuilder::new("debug-callback").code(code).build();

    let mut client = session.start();

    let result = client.execute(|executor| {
        executor.step_over()?;
        executor.step_over()?;
        executor.step_over()?;
        executor.step_over()?;
        Ok(())
    })?;

    let debug_output = DebugTestOutput::new(result);
    debug_output.assert_trace_snapshot_matches(
        "debugging/snapshots/test_if_over_numbers_with_second_matching.trace.txt",
    );

    Ok(())
}

#[test]
fn test_if_over_numbers_with_else_matching() -> anyhow::Result<()> {
    let code = r#"
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
"#;

    let session = DebugBuilder::new("debug-callback").code(code).build();

    let mut client = session.start();

    let result = client.execute(|executor| {
        executor.step_over()?;
        executor.step_over()?;
        executor.step_over()?;
        executor.step_over()?;
        executor.step_over()?;
        Ok(())
    })?;

    let debug_output = DebugTestOutput::new(result);
    debug_output.assert_trace_snapshot_matches(
        "debugging/snapshots/test_if_over_numbers_with_else_matching.trace.txt",
    );

    Ok(())
}

#[test]
fn test_inline_function_call_step_in() -> anyhow::Result<()> {
    let code = r#"
global foo: int;

fun my_sum(a: int, b: int): int {
    return a + b;
}

fun main() {
    foo = 300;
    return my_sum(foo, foo);
}
"#;

    let session = DebugBuilder::new("debug-callback").code(code).build();

    let mut client = session.start();

    let result = client.execute(|executor| {
        executor.step_in()?;
        executor.step_in()?;
        executor.step_in()?;
        executor.step_in()?;
        executor.step_in()?;
        executor.step_in()?;
        executor.step_in()?;
        Ok(())
    })?;

    let debug_output = DebugTestOutput::new(result);
    debug_output.assert_trace_snapshot_matches(
        "debugging/snapshots/test_inline_function_call_step_in.trace.txt",
    );

    Ok(())
}

#[test]
fn test_inline_function_call_step_over() -> anyhow::Result<()> {
    let code = r#"
global foo: int;

fun my_sum(a: int, b: int): int {
    return a + b;
}

fun main() {
    foo = 300;
    val goo = my_sum(foo, foo);
    return foo + goo;
}
"#;

    let session = DebugBuilder::new("debug-callback").code(code).build();

    let mut client = session.start();

    let result = client.execute(|executor| {
        executor.step_over()?;
        executor.step_over()?;
        Ok(())
    })?;

    let debug_output = DebugTestOutput::new(result);
    debug_output.assert_trace_snapshot_matches(
        "debugging/snapshots/test_inline_function_call_step_over.trace.txt",
    );

    Ok(())
}

#[test]
fn test_inline_function_call_step_out() -> anyhow::Result<()> {
    let code = r#"
global foo: int;

fun my_sum(a: int, b: int): int {
    return a + b;
}

fun main() {
    foo = 300;
    val goo = my_sum(foo, foo);
    return foo + goo;
}
"#;

    let session = DebugBuilder::new("debug-callback").code(code).build();

    let mut client = session.start();

    let result = client.execute(|executor| {
        executor.step_over()?;
        executor.step_in()?;
        executor.step_in()?;
        executor.step_in()?;
        executor.step_out()?;
        executor.step_over()?;
        Ok(())
    })?;

    let debug_output = DebugTestOutput::new(result);
    debug_output.assert_trace_snapshot_matches(
        "debugging/snapshots/test_inline_function_call_step_out.trace.txt",
    );

    Ok(())
}

#[test]
fn test_ref_inline_function_call_step_in() -> anyhow::Result<()> {
    let code = r#"
global foo: int;

@inline_ref
fun my_sum(a: int, b: int): int {
    return a + b;
}

fun main() {
    foo = 300;
    return my_sum(foo, foo);
}
"#;

    let session = DebugBuilder::new("debug-callback").code(code).build();

    let mut client = session.start();

    let result = client.execute(|executor| {
        executor.step_in()?;
        executor.step_in()?;
        executor.step_in()?;
        executor.step_in()?;
        executor.step_in()?;
        executor.step_in()?;
        executor.step_in()?;
        executor.step_in()?;
        executor.step_in()?;
        Ok(())
    })?;

    let debug_output = DebugTestOutput::new(result);
    debug_output.assert_trace_snapshot_matches(
        "debugging/snapshots/test_ref_inline_function_call_step_in.trace.txt",
    );

    Ok(())
}

#[test]
fn test_ref_inline_function_call_step_over() -> anyhow::Result<()> {
    let code = r#"
global foo: int;

@inline_ref
fun my_sum(a: int, b: int): int {
    return a + b;
}

fun main() {
    foo = 300;
    val result = my_sum(foo, foo);
    return result + foo;
}
"#;

    let session = DebugBuilder::new("debug-callback").code(code).build();

    let mut client = session.start();

    let result = client.execute(|executor| {
        executor.step_over()?;
        executor.step_over()?;
        Ok(())
    })?;

    let debug_output = DebugTestOutput::new(result);
    debug_output.assert_trace_snapshot_matches(
        "debugging/snapshots/test_ref_inline_function_call_step_over.trace.txt",
    );

    Ok(())
}

#[test]
fn test_ref_inline_function_call_step_out() -> anyhow::Result<()> {
    let code = r#"
global foo: int;

@inline_ref
fun my_sum(a: int, b: int): int {
    return a + b;
}

fun main() {
    foo = 300;
    val result = my_sum(foo, foo);
    return result + foo;
}
"#;

    let session = DebugBuilder::new("debug-callback").code(code).build();

    let mut client = session.start();

    let result = client.execute(|executor| {
        executor.step_over()?;
        executor.step_in()?;
        executor.step_in()?;
        executor.step_in()?;
        executor.step_out()?;
        executor.step_over()?;
        Ok(())
    })?;

    let debug_output = DebugTestOutput::new(result);
    debug_output.assert_trace_snapshot_matches(
        "debugging/snapshots/test_ref_inline_function_call_step_out.trace.txt",
    );

    Ok(())
}

#[test]
fn test_noinline_function_call_step_in() -> anyhow::Result<()> {
    let code = r#"
global foo: int;

@method_id(123)
fun my_sum(a: int, b: int): int {
    return a + b;
}

fun main() {
    foo = 300;
    return my_sum(foo, foo);
}
"#;

    let session = DebugBuilder::new("debug-callback").code(code).build();

    let mut client = session.start();

    let result = client.execute(|executor| {
        executor.step_in()?;
        executor.step_in()?;
        executor.step_in()?;
        executor.step_in()?;
        executor.step_in()?;
        executor.step_in()?;
        executor.step_in()?;
        executor.step_in()?;
        executor.step_in()?;
        Ok(())
    })?;

    let debug_output = DebugTestOutput::new(result);
    debug_output.assert_trace_snapshot_matches(
        "debugging/snapshots/test_noinline_function_call_step_in.trace.txt",
    );

    Ok(())
}

#[test]
fn test_noinline_function_call_step_over() -> anyhow::Result<()> {
    let code = r#"
global foo: int;

@method_id(123)
fun my_sum(a: int, b: int): int {
    return a + b;
}

fun main() {
    foo = 300;
    val res = my_sum(foo, foo);
    return res + foo;
}
"#;

    let session = DebugBuilder::new("debug-callback").code(code).build();

    let mut client = session.start();

    let result = client.execute(|executor| {
        executor.step_over()?;
        executor.step_over()?;
        Ok(())
    })?;

    let debug_output = DebugTestOutput::new(result);
    debug_output.assert_trace_snapshot_matches(
        "debugging/snapshots/test_noinline_function_call_step_over.trace.txt",
    );

    Ok(())
}

#[test]
fn test_noinline_function_call_step_out() -> anyhow::Result<()> {
    let code = r#"
global foo: int;

@method_id(123)
fun my_sum(a: int, b: int): int {
    return a + b;
}

fun main() {
    foo = 300;
    val res = my_sum(foo, foo);
    return res + foo;
}
"#;

    let session = DebugBuilder::new("debug-callback").code(code).build();

    let mut client = session.start();

    let result = client.execute(|executor| {
        executor.step_over()?;
        executor.step_in()?;
        executor.step_in()?;
        executor.step_in()?;
        executor.step_out()?;
        executor.step_over()?;
        Ok(())
    })?;

    let debug_output = DebugTestOutput::new(result);
    debug_output.assert_trace_snapshot_matches(
        "debugging/snapshots/test_noinline_function_call_step_out.trace.txt",
    );

    Ok(())
}
