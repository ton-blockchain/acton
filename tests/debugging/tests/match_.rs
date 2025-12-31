use crate::debugging::support::assertions::{DebugTestOutput, DebugTestOutputExt};
use crate::debugging::support::debug::DebugBuilder;

#[test]
fn test_match_over_numbers_with_first_matching() -> anyhow::Result<()> {
    let code = r#"
fun main(foo: int) {
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

    let session = DebugBuilder::new("debug-callback")
        .code(code)
        .accept_int(100)
        .build();

    let mut client = session.start();

    let result = client.execute(|executor| {
        executor.step_over()?;
        executor.step_over()?;
        Ok(())
    })?;

    let debug_output = DebugTestOutput::new(result);
    debug_output.assert_trace_snapshot_matches(
        "debugging/snapshots/match/over_numbers_with_first_matching.trace.txt",
    );

    Ok(())
}

#[test]
fn test_match_over_numbers_with_second_matching() -> anyhow::Result<()> {
    let code = r#"
fun main(foo: int) {
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

    let session = DebugBuilder::new("debug-callback")
        .code(code)
        .accept_int(200)
        .build();

    let mut client = session.start();

    // TODO
    let result = client.execute(|executor| {
        executor.step_over()?;
        executor.step_over()?;
        executor.step_over()?;
        executor.step_over()?;
        Ok(())
    })?;

    let debug_output = DebugTestOutput::new(result);
    debug_output.assert_trace_snapshot_matches(
        "debugging/snapshots/match/over_numbers_with_second_matching.trace.txt",
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
        "debugging/snapshots/match/over_numbers_with_else_matching.trace.txt",
    );

    Ok(())
}

#[test]
fn test_match_over_lazy_message_first_matching() -> anyhow::Result<()> {
    let code = r#"
struct (0x00000001) First {
    id: int32
}
struct (0x00000002) Second {
    data: bool
}

type Msg = First | Second

fun main() {
    val msg = lazy Msg.fromCell(First { id: 10 }.toCell());
    match (msg) {
        First => {
            return 10
        }
        Second => {
            return 20
        }
        else => {
            throw 0xFFFF
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
        executor.step_over()?;
        executor.step_over()?;
        executor.step_over()?;
        executor.step_over()?;
        executor.step_over()?;
        Ok(())
    })?;

    let debug_output = DebugTestOutput::new(result);
    debug_output.assert_trace_snapshot_matches(
        "debugging/snapshots/match/over_lazy_message_first_matching.trace.txt",
    );

    Ok(())
}

#[test]
fn test_match_over_lazy_message_second_matching() -> anyhow::Result<()> {
    let code = r#"
struct (0x00000001) First {
    id: int32
}
struct (0x00000002) Second {
    data: bool
}

type Msg = First | Second

fun main() {
    val msg = lazy Msg.fromCell(Second { data: true }.toCell());
    match (msg) {
        First => {
            return 10
        }
        Second => {
            return 20
        }
        else => {
            throw 0xFFFF
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
        executor.step_over()?;
        executor.step_over()?;
        executor.step_over()?;
        executor.step_over()?;
        executor.step_over()?;
        executor.step_over()?;
        executor.step_over()?;
        executor.step_over()?;
        executor.step_over()?;
        Ok(())
    })?;

    // TODO
    let debug_output = DebugTestOutput::new(result);
    debug_output.assert_trace_snapshot_matches(
        "debugging/snapshots/match/over_lazy_message_second_matching.trace.txt",
    );

    Ok(())
}

#[test]
fn test_match_over_lazy_message_else_matching() -> anyhow::Result<()> {
    let code = r#"
struct (0x00000001) First {
    id: int32
}
struct (0x00000002) Second {
    data: bool
}
struct (0x00000003) Third

type Msg = First | Second

fun main() {
    val msg = lazy Msg.fromCell(Third {}.toCell());
    match (msg) {
        First => {
            return 10
        }
        Second => {
            return 20
        }
        else => {
            throw 0xFFFF
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
        executor.step_over()?;
        executor.step_over()?;
        executor.step_over()?;
        executor.step_over()?;
        executor.step_over()?;
        executor.step_over()?;
        executor.step_over()?;
        executor.step_over()?;
        executor.step_over()?;
        executor.step_over()?;
        Ok(())
    })?;

    // TODO
    let debug_output = DebugTestOutput::new(result);
    debug_output.assert_trace_snapshot_matches(
        "debugging/snapshots/match/over_lazy_message_else_matching.trace.txt",
    );

    Ok(())
}
