use crate::support::debugger::debug::DebugBuilder;
use anyhow::bail;
use std::fs;
use tempfile::tempdir;
use tolk_compiler::{Compiler, CompilerResult};
use ton::block_tlb::StateInit;
use ton::ton_core::cell::TonCell;
use ton::ton_core::traits::tlb::TLB;
use tvm_ffi::stack::{Tuple, TupleItem};
use tycho_types::models::{StdAddr, StdAddrFormat};

fn compile_contract_address(code: &str) -> anyhow::Result<String> {
    let temp_dir = tempdir()?;
    let path = temp_dir.path().join("main.tolk");
    fs::write(&path, code)?;

    let compiled = match Compiler::new(2).compile(&path, true) {
        CompilerResult::Success(result) => result,
        CompilerResult::Error(error) => bail!("Cannot compile test script: {}", error.message),
    };
    let code_cell = TonCell::from_boc_base64(&compiled.code_boc64)?;
    let address = StateInit::new(code_cell, TonCell::empty().clone()).derive_address(0)?;
    let (address, _) = StdAddr::from_str_ext(&address.to_string(), StdAddrFormat::any())?;
    Ok(address.to_string())
}

#[test]
fn test_evaluate_zero_arg_function_call() -> anyhow::Result<()> {
    let code = r"
fun helper(): int {
    return 42;
}

fun main() {
    val value = 1;
    return value;
}
";

    let session = DebugBuilder::new("debug-evaluate-zero-arg")
        .code(code)
        .build();
    let mut client = session.start();

    let _result = client.execute(|executor| {
        let value = executor.evaluate("helper()")?;
        snapbox::assert_data_eq!(value.result, snapbox::str!["42"]);
        Ok(())
    })?;

    Ok(())
}

#[test]
fn test_evaluate_one_arg_function_call() -> anyhow::Result<()> {
    let code = r"
fun helper(x: int): int {
    return x + 1;
}

fun main(value: int) {
    return value;
}
";

    let session = DebugBuilder::new("debug-evaluate-one-arg")
        .code(code)
        .accept_int(41)
        .build();
    let mut client = session.start();

    let _result = client.execute(|executor| {
        let value = executor.evaluate("helper(value)")?;
        snapbox::assert_data_eq!(value.result, snapbox::str!["42"]);
        Ok(())
    })?;

    Ok(())
}

#[test]
fn test_evaluate_literal_argument_function_call() -> anyhow::Result<()> {
    let code = r"
fun helper(x: int): int {
    return x + 1;
}

fun main() {
    return 0;
}
";

    let session = DebugBuilder::new("debug-evaluate-literal-arg")
        .code(code)
        .build();
    let mut client = session.start();

    let _result = client.execute(|executor| {
        let value = executor.evaluate("helper(41)")?;
        snapbox::assert_data_eq!(value.result, snapbox::str!["42"]);
        Ok(())
    })?;

    Ok(())
}

#[test]
fn test_evaluate_multi_arg_function_call() -> anyhow::Result<()> {
    let code = r"
fun helper(a: int, b: int): int {
    return a + b;
}

fun main(left: int, right: int) {
    return left + right;
}
";

    let session = DebugBuilder::new("debug-evaluate-multi-arg")
        .code(code)
        .stack(Tuple(vec![
            TupleItem::Int(20.into()),
            TupleItem::Int(22.into()),
        ]))
        .build();
    let mut client = session.start();

    let _result = client.execute(|executor| {
        let value = executor.evaluate("helper(left, right)")?;
        snapbox::assert_data_eq!(value.result, snapbox::str!["42"]);
        Ok(())
    })?;

    Ok(())
}

#[test]
fn test_evaluate_mixed_local_and_literal_arguments() -> anyhow::Result<()> {
    let code = r"
fun helper(a: int, b: int): int {
    return a + b;
}

fun main(value: int) {
    return value;
}
";

    let session = DebugBuilder::new("debug-evaluate-mixed-args")
        .code(code)
        .accept_int(41)
        .build();
    let mut client = session.start();

    let _result = client.execute(|executor| {
        let value = executor.evaluate("helper(value, 1)")?;
        snapbox::assert_data_eq!(value.result, snapbox::str!["42"]);
        Ok(())
    })?;

    Ok(())
}

#[test]
fn test_evaluate_field_access_argument_function_call() -> anyhow::Result<()> {
    let code = r"
struct BoxedInt {
    value: int,
}

fun helper(x: int): int {
    return x + 1;
}

fun main() {
    val boxed = BoxedInt { value: 41 };
    return boxed.value;
}
";

    let session = DebugBuilder::new("debug-evaluate-field-arg")
        .code(code)
        .build();
    let mut client = session.start();

    let _result = client.execute(|executor| {
        executor.step_over()?;
        let value = executor.evaluate("helper(boxed.value)")?;
        snapbox::assert_data_eq!(value.result, snapbox::str!["42"]);
        Ok(())
    })?;

    Ok(())
}

#[test]
fn test_evaluate_field_access_argument_on_lazy_struct_function_call() -> anyhow::Result<()> {
    let code = r"
struct LazyBoxedInt {
    value: uint32,
    other: uint32,
}

fun helper(x: uint32): uint32 {
    return x + 1;
}

fun main() {
    val packed = LazyBoxedInt { value: 41, other: 999 }.toCell().beginParse();
    val boxed = lazy LazyBoxedInt.fromSlice(packed);
    val current = boxed.value;
    return current;
}
";

    let session = DebugBuilder::new("debug-evaluate-lazy-field-arg")
        .code(code)
        .build();
    let mut client = session.start();

    let _result = client.execute(|executor| {
        executor.step_over_times(3)?;
        let value = executor.evaluate("helper(boxed.value)")?;
        snapbox::assert_data_eq!(value.result, snapbox::str!["42"]);
        Ok(())
    })?;

    Ok(())
}

#[test]
fn test_evaluate_multiline_function_call_with_trailing_comma() -> anyhow::Result<()> {
    let code = r"
struct BoxedInt {
    value: int,
}

fun calcDeployedJettonWallet(a: int, b: int, c: int): int {
    return a + b + c;
}

fun main() {
    val boxed = BoxedInt { value: 14 };
    return boxed.value;
}
";

    let session = DebugBuilder::new("debug-evaluate-multiline-call")
        .code(code)
        .build();
    let mut client = session.start();

    let _result = client.execute(|executor| {
        executor.step_over()?;
        let value = executor.evaluate(
            "calcDeployedJettonWallet(
                boxed.value,
                boxed.value,
                14,
            )",
        )?;
        snapbox::assert_data_eq!(value.result, snapbox::str!["42"]);
        Ok(())
    })?;

    Ok(())
}

#[test]
fn test_evaluate_contract_get_address_from_c7() -> anyhow::Result<()> {
    let code = r"
fun ownAddress(): address {
    return contract.getAddress();
}

fun main() {
    return 0;
}
";
    let expected_address = compile_contract_address(code)?;

    let session = DebugBuilder::new("debug-evaluate-contract-get-address")
        .code(code)
        .build();
    let mut client = session.start();

    let _result = client.execute(|executor| {
        let value = executor.evaluate("contract.getAddress()")?;
        snapbox::assert_data_eq!(
            format!(
                "{}: {}",
                value.type_field.unwrap_or_default(),
                value.result == expected_address
            ),
            snapbox::str!["address: true"],
        );
        snapbox::assert_data_eq!(
            (executor.evaluate("ownAddress()")?.result == expected_address).to_string(),
            snapbox::str!["true"],
        );
        Ok(())
    })?;

    Ok(())
}

#[test]
fn test_evaluate_restores_current_registers_and_globals() -> anyhow::Result<()> {
    let session = DebugBuilder::new("debug-evaluate-runtime-snapshot")
        .code(
            r#"
global current: int

fun getC7(): tuple asm "c7 PUSHCTR"
fun setC7(value: tuple): void asm "c7 POPCTR"
fun getActions(): cell asm "c5 PUSHCTR"
fun setActions(value: cell): void asm "c5 POPCTR"
fun readNow(): int asm "NOW"

fun stateValue(): int {
    return current + contract.getData().beginParse().loadInt(8);
}

fun mutateState(): int {
    current = 90;
    contract.setData(beginCell().storeInt(9, 8).endCell());
    return stateValue();
}

@noinline
fun paused(value: int): int {
    return value;
}

fun main(): int {
    current = 40;
    contract.setData(beginCell().storeInt(2, 8).endCell());
    setActions(beginCell().storeInt(3, 8).endCell());
    var c7 = getC7();
    var info = c7.get(0) as tuple;
    info.set(123, 3);
    c7.set(info, 0);
    setC7(c7);
    return paused(current);
}
"#,
        )
        .build();

    session.start().execute(|executor| {
        executor.step_in_until_function("paused")?;
        snapbox::assert_data_eq!(
            executor.evaluate("stateValue()")?.result,
            snapbox::str!["42"]
        );
        snapbox::assert_data_eq!(executor.evaluate("readNow()")?.result, snapbox::str!["123"]);
        snapbox::assert_data_eq!(
            executor
                .evaluate("getActions().beginParse().loadInt(8)")?
                .result,
            snapbox::str!["3"],
        );
        snapbox::assert_data_eq!(
            executor.evaluate("mutateState()")?.result,
            snapbox::str!["99"]
        );
        snapbox::assert_data_eq!(
            executor.evaluate("stateValue()")?.result,
            snapbox::str!["42"]
        );
        executor.step_in_until_terminated(20)?;
        Ok(())
    })?;
    Ok(())
}

#[test]
fn test_evaluate_generic_struct_result_and_repeated_calls() -> anyhow::Result<()> {
    let session = DebugBuilder::new("debug-evaluate-struct-result")
        .code(
            r"
struct Box<T> {
    value: T
}

fun boxed(value: int): Box<int> {
    return Box { value: value + 1 };
}

fun unbox(box: Box<int>): int { return box.value; }

fun main(value: int) {
    return value;
}
",
        )
        .accept_int(41)
        .build();
    session.start().execute(|executor| {
        snapbox::assert_data_eq!(
            executor.evaluate_fields("boxed(value)")?,
            snapbox::str![[r"
Box<int>:
value: int = 42
"]]
        );
        snapbox::assert_data_eq!(
            executor.evaluate_fields("boxed(1)")?,
            snapbox::str![[r"
Box<int>:
value: int = 2
"]]
        );
        snapbox::assert_data_eq!(executor.evaluate("value")?.result, snapbox::str!["41"]);
        snapbox::assert_data_eq!(
            executor.evaluate("unbox({ value })")?.result,
            snapbox::str!["41"]
        );
        Ok(())
    })?;
    Ok(())
}

#[test]
fn test_evaluate_runtime_tuple_and_alias_fields() -> anyhow::Result<()> {
    let session = DebugBuilder::new("debug-evaluate-tuple-fields")
        .code(
            r"
struct Box<T> { value: T }
type Wrapped<T> = Box<T>

fun increment(value: int): int { return value + 1; }

fun main(value: int) {
    val wrapped: Wrapped<int> = { value };
    val pair: [int, int] = [17, value];
    return wrapped.value + pair.1;
}
",
        )
        .accept_int(41)
        .build();
    session.start().execute(|executor| {
        executor.step_over_times(2)?;
        snapbox::assert_data_eq!(
            executor.evaluate("increment(wrapped.value)")?.result,
            snapbox::str!["42"]
        );
        snapbox::assert_data_eq!(
            executor.evaluate("increment(pair.1)")?.result,
            snapbox::str!["42"]
        );
        Ok(())
    })?;
    Ok(())
}

#[test]
fn test_evaluate_transfer_body_uses_runtime_types_for_abi_client_fields() -> anyhow::Result<()> {
    let session = DebugBuilder::new("debug-evaluate-transfer-body")
        .code(
            r#"
type ForwardPayloadRemainder = RemainingBitsAndRefs

struct (0b0) PayloadInline { value: RemainingBitsAndRefs }
struct (0b1) PayloadInRef { value: Cell<RemainingBitsAndRefs> }

struct (0x0f8a7ea5) AskToTransfer {
    queryId: uint64
    jettonAmount: coins
    transferRecipient: address
    sendExcessesTo: address?
    customPayload: cell?
    forwardTonAmount: coins
    @abi.clientType(PayloadInline | PayloadInRef)
    forwardPayload: ForwardPayloadRemainder
}

struct Account { address: address }

fun buildTransferBody(
    jettonAmount: int,
    forwardTonAmount: int,
    transferRecipient: address,
    sendExcessesTo: address,
    forwardPayload: slice,
    customPayload: cell? = null,
): AskToTransfer {
    return AskToTransfer {
        queryId: 0,
        jettonAmount,
        transferRecipient,
        sendExcessesTo,
        customPayload,
        forwardTonAmount,
        forwardPayload,
    };
}

@noinline
fun paused(deployer: Account, badPayload: slice): int {
    return badPayload.remainingBitsCount();
}

fun main(): int {
    val deployer = Account {
        address: address("0:0000000000000000000000000000000000000000000000000000000000000000"),
    };
    val badPayload = beginCell()
        .storeUint(1, 1)
        .storeRef(beginCell().endCell())
        .storeUint(1, 1)
        .endCell()
        .beginParse();
    return paused(deployer, badPayload);
}
"#,
        )
        .build();

    session.start().execute(|executor| {
        executor.step_in_until_function("paused")?;
        snapbox::assert_data_eq!(
            executor.evaluate_fields(
                "buildTransferBody(grams(\"1\"), 0, deployer.address, deployer.address, badPayload)",
            )?,
            snapbox::str![[r"
AskToTransfer:
queryId: uint64 = 0
jettonAmount: coins = 1000000000
transferRecipient: address = 0:0000000000000000000000000000000000000000000000000000000000000000
sendExcessesTo: address? = 0:0000000000000000000000000000000000000000000000000000000000000000
customPayload: cell? = null
forwardTonAmount: coins = 0 GRAM
forwardPayload: RemainingBitsAndRefs = 2 bits, 1 refs, hash: [..]
"]],
        );
        snapbox::assert_data_eq!(
            executor.evaluate("badPayload.remainingBitsCount()")?.result,
            snapbox::str!["2"],
        );
        executor.step_in_until_terminated(20)?;
        Ok(())
    })?;
    Ok(())
}

#[test]
fn test_evaluate_preserves_slice_cursor() -> anyhow::Result<()> {
    let session = DebugBuilder::new("debug-evaluate-slice-cursor")
        .code(
            r"
fun readNext(value: slice): int { return value.loadUint(8); }

fun main() {
    var data = beginCell().storeUint(17, 8).storeUint(42, 8).endCell().beginParse();
    val first = data.loadUint(8);
    return first + data.loadUint(8);
}
",
        )
        .build();
    session.start().execute(|executor| {
        executor.step_over_times(2)?;
        snapbox::assert_data_eq!(
            executor.evaluate("readNext(data)")?.result,
            snapbox::str!["42"]
        );
        snapbox::assert_data_eq!(
            executor.evaluate("readNext(data)")?.result,
            snapbox::str!["42"]
        );
        Ok(())
    })?;
    Ok(())
}

#[test]
fn test_evaluate_errors_leave_session_usable() -> anyhow::Result<()> {
    let session = DebugBuilder::new("debug-evaluate-errors")
        .code(
            r"
fun increment(value: int): int { return value + 1; }
fun fail(): int { throw 123; }
fun main(value: int) { return value; }
",
        )
        .accept_int(41)
        .build();
    session.start().execute(|executor| {
        for expression in ["increment(", "value +"] {
            snapbox::assert_data_eq!(
                executor.evaluate(expression).unwrap_err().to_string(),
                snapbox::str!["syntax error"],
            );
        }

        let missing = executor
            .evaluate("increment(missing)")
            .unwrap_err()
            .to_string();
        snapbox::assert_data_eq!(
            format!(
                "compile error: {}, missing name: {}",
                missing.contains("failed to compile"),
                missing.contains("missing")
            ),
            snapbox::str!["compile error: true, missing name: true"],
        );
        snapbox::assert_data_eq!(
            executor.evaluate("increment(value + 1)")?.result,
            snapbox::str!["43"]
        );
        let compile_error = executor
            .evaluate("unknownFunction()")
            .unwrap_err()
            .to_string();
        snapbox::assert_data_eq!(
            compile_error.contains("failed to compile").to_string(),
            snapbox::str!["true"]
        );
        let vm_error = executor.evaluate("fail()").unwrap_err().to_string();
        snapbox::assert_data_eq!(
            vm_error.contains("VM exit code 123").to_string(),
            snapbox::str!["true"]
        );
        snapbox::assert_data_eq!(
            executor.evaluate("increment(value)")?.result,
            snapbox::str!["42"]
        );
        snapbox::assert_data_eq!(
            executor.evaluate("((increment(value)))")?.result,
            snapbox::str!["42"]
        );
        snapbox::assert_data_eq!(executor.evaluate("(value)")?.result, snapbox::str!["41"]);
        Ok(())
    })?;
    Ok(())
}

#[test]
fn test_evaluate_in_cli_script_with_relative_import() -> anyhow::Result<()> {
    use crate::support::debugger::debug::CliDebugBuilder;
    use crate::support::project::ProjectBuilder;

    let project = ProjectBuilder::new("evaluate-cli-script")
        .file(
            "helpers",
            "fun helper(value: int): int { return value + 1; }",
        )
        .file(
            "main",
            r#"
import "helpers"

fun main() {
    val value = 41;
    return value;
}
"#,
        )
        .build();
    CliDebugBuilder::script(project, "main.tolk")
        .build()
        .start()
        .execute(|executor| {
            executor.step_over()?;
            snapbox::assert_data_eq!(
                executor.evaluate("helper(value)")?.result,
                snapbox::str!["42"]
            );
            Ok(())
        })?;
    Ok(())
}

#[test]
fn test_evaluate_uses_selected_call_frame() -> anyhow::Result<()> {
    let session = DebugBuilder::new("debug-evaluate-selected-frame")
        .code(
            r"
fun increment(value: int): int { return value + 1; }

@noinline
fun inner(value: int): int { return value; }

fun main(value: int): int { return inner(value + 1); }
",
        )
        .accept_int(41)
        .build();
    session.start().execute(|executor| {
        executor.step_in_until_function("inner")?;
        snapbox::assert_data_eq!(
            executor.evaluate_in_frame("increment(value)", 0)?.result,
            snapbox::str!["43"]
        );
        snapbox::assert_data_eq!(
            executor.evaluate_in_frame("increment(value)", 1)?.result,
            snapbox::str!["42"]
        );
        Ok(())
    })?;
    Ok(())
}

#[test]
fn test_evaluate_method_chains_with_runtime_arguments() -> anyhow::Result<()> {
    let session = DebugBuilder::new("debug-evaluate-method-chains")
        .code(
            r"
fun increment(value: int): int { return value + 1; }

fun main(value: int): int {
    var b = beginCell();
    return b.storeInt(value, 8).endCell().beginParse().loadInt(8);
}
",
        )
        .accept_int(41)
        .build();
    session.start().execute(|executor| {
        snapbox::assert_data_eq!(
            executor
                .evaluate("beginCell().storeInt(1, 2)")?
                .type_field
                .unwrap_or_default(),
            snapbox::str!["builder"],
        );
        snapbox::assert_data_eq!(
            executor
                .evaluate("beginCell().storeInt(1, 2).endCell().beginParse().loadInt(2)")?
                .result,
            snapbox::str!["1"],
        );
        snapbox::assert_data_eq!(
            executor
                .evaluate(
                    "beginCell().storeInt(increment(value), 8).endCell().beginParse().loadInt(8)"
                )?
                .result,
            snapbox::str!["42"],
        );
        executor.step_over()?;
        snapbox::assert_data_eq!(
            executor
                .evaluate("b.storeInt(value + 1, 8).endCell().beginParse().loadInt(8)")?
                .result,
            snapbox::str!["42"],
        );
        snapbox::assert_data_eq!(
            executor
                .evaluate("b.endCell().beginParse().isEmpty()")?
                .result,
            snapbox::str!["true"],
        );
        Ok(())
    })?;
    Ok(())
}
