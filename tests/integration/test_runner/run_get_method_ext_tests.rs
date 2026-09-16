use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const GETTER_TYPES: &str = r"
struct Calculation {
    value: int
    positive: bool
}
";

const GETTER: &str = r#"
import "getter_types"

fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}

enum GetterError {
    Failed = 123
}

get fun calculate(count: int): Calculation {
    var value = 0;
    var i = 0;
    while (i < count) {
        value += i;
        i += 1;
    }
    return { value, positive: value > 0 };
}

get fun empty(): void {}

get fun nullable(): int? {
    return null;
}

get fun nested(): [int, int] {
    return [11, 22];
}

fun alternativeReturn(): int asm "42 PUSHINT" "RETALT"

get fun alternative(): int {
    return alternativeReturn();
}

get fun fail(): int {
    throw GetterError.Failed;
}
"#;

const SETUP: &str = r#"
import "../../lib/build"
import "../../lib/emulation/network"
import "../../lib/emulation/testing"
import "../../lib/io"
import "../../lib/testing/expect"
import "../contracts/getter_types"

fun deployGetter(): address {
    val sender = testing.treasury("sender");
    val init = ContractState {
        code: build("getter"),
        data: createEmptyCell(),
    };
    val addr = AutoDeployAddress { stateInit: init }.calculateAddress();
    net.send(sender.address, createMessage({
        bounce: false,
        value: ton("1"),
        dest: { stateInit: init },
    }));
    return addr;
}
"#;

fn getter_project(name: &str, body: &str) -> ProjectBuilder {
    ProjectBuilder::new(name)
        .file("contracts/getter_types", GETTER_TYPES)
        .contract("getter", GETTER)
        .test_file("get_method_meta", &format!("{SETUP}\n{body}"))
}

#[test]
fn run_get_method_ext_returns_typed_results_and_execution_metadata() {
    getter_project(
        "get-method-ext-results",
        r#"
get fun `test get method metadata`() {
    val addr = deployGetter();
    val execution = net.runGetMethodExt<Calculation>(addr, "calculate", [4]);
    expect(execution.isSuccess()).toEqual(true);
    expect(execution).toHaveExitCode(0);
    val result = execution.unwrap();
    val meta = execution.meta;
    println(result);
    println(meta);

    // 70519 is calculate's method ID. Both entrypoints must execute the same method.
    val byId = net.runGetMethodExt<Calculation>(addr, 70519, [4]);
    expect(byId.unwrap()).toEqual(result);
    expect(byId.meta).toEqual(meta);
    expect(net.runGetMethod<Calculation>(addr, "calculate", [4])).toEqual(result);

    val larger = net.runGetMethodExt<Calculation>(addr, "calculate", [8]);
    println(larger.unwrap());
    println(larger.meta);
    expect(larger.meta.gasUsed > meta.gasUsed).toEqual(true);

    // A later call must not overwrite metadata returned by an earlier invocation.
    expect(execution.meta).toEqual(byId.meta);
    expect(execution.unwrap()).toEqual(result);
    val empty = net.runGetMethodExt<void>(addr, "empty");
    empty.unwrap();
    println(empty.isSuccess());
    println(empty.meta);
    net.runGetMethod<void>(addr, "empty");

    val nullable = net.runGetMethodExt<int?>(addr, "nullable");
    println(nullable.unwrap());
    println(nullable.meta);
    expect(nullable.isSuccess()).toEqual(true);
    expect(nullable.unwrap()).toBeNull();
    expect(net.runGetMethod<int?>(addr, "nullable")).toBeNull();

    val nested = net.runGetMethodExt<[int, int]>(addr, "nested");
    println(nested.unwrap());
    println(nested.meta);
    expect(nested.unwrap()).toEqual([11, 22]);

    val alternative = net.runGetMethodExt<int>(addr, "alternative");
    println(alternative.unwrap());
    println(alternative.meta);
    expect(alternative.unwrap()).toEqual(42);
    expect(alternative.isSuccess()).toEqual(true);
    expect(alternative).toHaveExitCode(1);

    // Unwrap must use the saved stack even after the contract no longer exists.
    testing.setShardAccount(addr, null);
    expect(execution.unwrap()).toEqual(result);
    expect(execution.unwrap()).toEqual(result);
    expect(execution.meta).toEqual(meta);
}
"#,
    )
    .build()
    .acton()
    .test()
    .run()
    .success()
    .assert_passed(1)
    .assert_snapshot_matches(
        "integration/snapshots/test-runner/run_get_method_ext/results.stdout.txt",
    );
}

#[test]
fn run_get_method_ext_returns_metadata_on_vm_failure() {
    getter_project(
        "get-method-ext-failure",
        r#"
get fun `test failed get method returns metadata`() {
    val addr = deployGetter();
    // A failed VM stack contains exception data, not a serialized Calculation.
    val execution = net.runGetMethodExt<Calculation>(addr, "fail");
    val meta = execution.meta;
    expect(execution.isSuccess()).toEqual(false);
    expect(execution).toHaveExitCode(123);
    expect(meta.gasUsed > 0).toEqual(true);
    println(execution.isSuccess());
    println(meta);

    val byId = net.runGetMethodExt<int>(addr, 118408);
    expect(byId.isSuccess()).toEqual(false);
    expect(byId.meta).toEqual(meta);

    val empty = net.runGetMethodExt<void>(addr, "fail");
    expect(empty.isSuccess()).toEqual(false);
    expect(empty.meta).toEqual(meta);

    val nullable = net.runGetMethodExt<int?>(addr, "fail");
    expect(nullable.isSuccess()).toEqual(false);
    expect(nullable.meta).toEqual(meta);

    val missing = net.runGetMethodExt<int>(addr, "missing");
    expect(missing.isSuccess()).toEqual(false);
    expect(missing).toHaveExitCode(11);
    println(missing.isSuccess());
    println(missing.meta);

    // Handled VM failures must not poison the next getter or the test result.
    val next = net.runGetMethodExt<Calculation>(addr, "calculate", [4]);
    expect(next.unwrap()).toEqual(net.runGetMethod<Calculation>(addr, "calculate", [4]));
    expect(next).toHaveExitCode(0);
    expect(execution).toHaveExitCode(123);
    expect(meta).toEqual(byId.meta);
    println(next.unwrap());
    println(next.meta);
}
"#,
    )
    .build()
    .acton()
    .test()
    .run()
    .success()
    .assert_passed(1)
    .assert_snapshot_matches(
        "integration/snapshots/test-runner/run_get_method_ext/getter_failure.stdout.txt",
    );
}

#[test]
fn run_get_method_ext_preserves_missing_contract_and_code_failures() {
    getter_project(
        "get-method-ext-preconditions",
        r#"
get fun `test undeployed contract still fails`() {
    val addr = randomAddress("undeployed");
    println(net.runGetMethodExt<int>(addr, "calculate", [4]));
}

get fun `test contract without code still fails`() {
    val sender = testing.treasury("sender");
    val dest = AutoDeployAddress {
        stateInit: beginCell()
            .storeBool(false) // fixed_prefix_length
            .storeBool(false) // special
            .storeBool(false) // code
            .storeBool(false) // data
            .storeBool(false) // library
            .endCell(),
    };
    net.send(sender.address, createMessage({
        bounce: false,
        value: ton("1"),
        dest,
    }));
    println(net.runGetMethodExt<int>(dest.calculateAddress(), "calculate", [4]));
}
"#,
    )
    .build()
    .acton()
    .test()
    .run()
    .failure()
    .assert_failed(2)
    .assert_snapshot_matches(
        "integration/snapshots/test-runner/run_get_method_ext/precondition_failures.stdout.txt",
    );
}

#[test]
fn run_get_method_ext_defers_result_type_failure_until_unwrap() {
    getter_project(
        "get-method-ext-invalid-result",
        r#"
get fun `test metadata is available before decoding`() {
    val addr = deployGetter();
    val result = net.runGetMethodExt<int>(addr, "empty");
    expect(result.isSuccess()).toEqual(true);
    expect(result).toHaveExitCode(0);
    println(result.meta);
    result.unwrap();
    println("unreachable");
    println(result);
}
"#,
    )
    .build()
    .acton()
    .test()
    .run()
    .failure()
    .assert_failed(1)
    .assert_snapshot_matches(
        "integration/snapshots/test-runner/run_get_method_ext/result_type_failure.stdout.txt",
    );
}

#[test]
fn run_get_method_ext_reports_diagnostics_for_the_selected_execution() {
    getter_project(
        "get-method-ext-diagnostics",
        r#"
fun __unwrapSavedResult<Ret>(execution: GetMethodResult<Ret>): Ret {
    return execution.unwrap();
}

fun read__savedValue<Ret>(execution: GetMethodResult<Ret>): Ret {
    return __unwrapSavedResult(execution);
}

get fun `test unwrap reports the original getter failure`() {
    val addr = deployGetter();
    val failed = net.runGetMethodExt<Calculation>(addr, "fail");
    expect(net.runGetMethodExt<int>(addr, "missing")).toHaveExitCode(11);
    net.runGetMethod<Calculation>(addr, "calculate", [4]);
    // Hide __-prefixed helpers, but keep public names that contain __ in the middle.
    read__savedValue(failed);
}

get fun `test mismatch reports the failed getter and expected code`() {
    val addr = deployGetter();
    val failed = net.runGetMethodExt<int>(addr, "fail");
    net.runGetMethod<Calculation>(addr, "calculate", [4]);
    expect(failed).toHaveExitCode(124);
}

get fun `test mismatch reports a successful getter`() {
    val addr = deployGetter();
    val success = net.runGetMethodExt<Calculation>(addr, "calculate", [4]);
    expect(net.runGetMethodExt<int>(addr, "fail")).toHaveExitCode(123);
    expect(success).toHaveExitCode(123);
}
"#,
    )
    .build()
    .acton()
    .test()
    .with_backtrace("full")
    .run()
    .failure()
    .assert_failed(3)
    .assert_snapshot_matches(
        "integration/snapshots/test-runner/run_get_method_ext/diagnostics.stdout.txt",
    );
}
