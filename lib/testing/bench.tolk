/// Utilities for measuring and profiling Tolk callbacks in tests.

import "@stdlib/reflection"
import "assert"
import "../ffi"
import "../fmt"

/// Root namespace for benchmark helpers.
struct bench

/// Aggregated gas statistics for a single TVM instruction mnemonic executed by
/// the measured callback.
///
/// The profiler groups samples by instruction mnemonic. For example, all `LDI`
/// executions are merged into one `BenchInstructionInfo` row with the total
/// number of executions and min/max gas observed for that mnemonic.
struct BenchInstructionInfo {
    /// Row id in the profile map.
    id: uint32
    /// TVM instruction mnemonic, for example `LDI`, `CALLDICT`, or `implicit RET`.
    name: string
    /// Number of times this instruction mnemonic was executed.
    count: uint32
    /// Sum of gas used by all executions of this instruction mnemonic.
    totalGas: uint64
    /// Minimum gas used by one execution of this instruction mnemonic.
    minGas: uint64
    /// Maximum gas used by one execution of this instruction mnemonic.
    maxGas: uint64
}

/// Result of `bench.measure`.
///
/// Example:
/// ```tolk
/// val profile = bench.measure("sum loop", fun() {
///     return calculateSum();
/// });
/// println(bench.format(profile));
/// println(profile.asmCode);
/// ```
struct BenchResult<Ret> {
    /// Benchmark name passed as the first argument to `bench.measure`.
    name: string
    /// Callback return value deserialized as `Ret`.
    result: Ret
    /// Total gas attributed to the callback body.
    gasUsed: uint64
    /// Per-instruction gas profile keyed by row id.
    instructions: map<uint32, BenchInstructionInfo>
    /// TASM code of the measured callback body.
    asmCode: string
}

/// Measures a zero-argument callback and returns its result, total gas,
/// per-instruction gas statistics, and the callback body TASM code.
///
/// Use a short descriptive `name`; it appears in formatted profile and diff
/// output. Close over data in the surrounding test when you need inputs.
///
/// `location` is filled automatically and is used in failure messages.
///
/// Example:
/// ```tolk
/// get fun `test bench parser`() {
///     val source = buildTestSlice();
///     val profile = bench.measure("fromSlice", fun() {
///         return Foo.fromSlice(source);
///     });
///     expect(profile.result.a).toEqual(1);
///     println(bench.format(profile));
/// }
/// ```
fun bench.measure<Ret>(
    name: string,
    fn: () -> Ret,
    location: string = reflect.sourceLocationAsString(),
): BenchResult<Ret> {
    val hostResult = ffi.benchMeasure(
        fn as unknown as continuation,
        bench.__baseline as unknown as continuation,
        [],
        contract.getAddress(),
        location,
    );
    val rawResult = hostResult.get(0) as array<unknown>;
    try {
        return BenchResult<Ret> {
            name: name,
            result: Ret.fromTuple(rawResult),
            gasUsed: hostResult.get(1) as uint64,
            instructions: hostResult.get(2) as map<uint32, BenchInstructionInfo>,
            asmCode: hostResult.get(3) as string,
        };
    } catch (e) {
        if (e == 7) {
            Assert.fail(
                format(
                    "bench.measure returned '{}' that cannot be deserialized to '{}'.",
                    rawResult,
                    reflect.typeNameOf<Ret>(),
                ),
            );
        }
        throw e;
    }
}

fun bench.__baseline(): void {}

/// Formats a table with per-instruction statistics for one measured profile.
///
/// The table includes total gas, average gas, min/max gas, and execution count
/// for each instruction mnemonic.
///
/// Example:
/// ```tolk
/// val profile = bench.measure("sum loop", fun() {
///     return calculateSum();
/// });
/// println(bench.format(profile));
/// ```
fun bench.format<Ret>(profile: BenchResult<Ret>): string {
    return ffi.benchFormat(profile.name, profile.gasUsed, profile.instructions);
}

/// Formats a diff table. The first profile is the baseline, the second is current.
///
/// Use this when comparing two implementations of the same behavior. Positive
/// gas diff means the current profile is more expensive; negative diff means it
/// is cheaper.
///
/// Example:
/// ```tolk
/// val baseline = bench.measure("fromSlice", fun() {
///     return Foo.fromSlice(source);
/// });
/// val current = bench.measure("manual parsing", fun() {
///     return parseFooManually(source);
/// });
/// println(bench.formatDiff(baseline, current));
/// ```
fun bench.formatDiff<BaselineRet, CurrentRet>(
    baseline: BenchResult<BaselineRet>,
    current: BenchResult<CurrentRet>,
): string {
    return ffi.benchFormatDiff(
        baseline.name,
        baseline.gasUsed,
        baseline.instructions,
        current.name,
        current.gasUsed,
        current.instructions,
    );
}

fun ffi.benchMeasure(
    functionToCall: continuation,
    baselineToCall: continuation,
    args: array<unknown>,
    on: address,
    location: string,
): tuple
    asm "300 EXTCALL"

fun ffi.benchFormat(
    name: string,
    gasUsed: uint64,
    instructions: map<uint32, BenchInstructionInfo>,
): string
    asm "301 EXTCALL"

fun ffi.benchFormatDiff(
    baselineName: string,
    baselineGasUsed: uint64,
    baselineInstructions: map<uint32, BenchInstructionInfo>,
    currentName: string,
    currentGasUsed: uint64,
    currentInstructions: map<uint32, BenchInstructionInfo>,
): string
    asm "302 EXTCALL"
