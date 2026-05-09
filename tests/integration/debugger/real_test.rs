use crate::support::debugger::debug::{CliDebugBuilder, DebugBuilder, DebugSession};
use crate::support::project::ProjectBuilder;

const COUNTER: &str = r#"import "../counter_messages"

contract Counter {
    storage: Storage
    incomingMessages: AllowedMessage
}

type AllowedMessage = IncreaseCounter | ResetCounter

fun handleIncreaseCounter(increaseBy: int) {
    var storage = lazy Storage.load();
    storage.counter += increaseBy;
    storage.save();
}

global a: int
global b: int

@noinline
fun heavy(increaseBy: int) {
    a = increaseBy;
    b = increaseBy;

    inner(increaseBy);

    a = a + b;
}

@noinline
fun inner(increaseBy: int) {
    b = increaseBy;
}

fun onInternalMessage(in: InMessage) {
    val msg = lazy AllowedMessage.fromSlice(in.body);

    debug.printString("Hello World");

    match (msg) {
        IncreaseCounter => {
            handleIncreaseCounter(msg.increaseBy);
            heavy(msg.increaseBy);
        }

        ResetCounter => {
            var storage = lazy Storage.load();
            storage.counter = 0;
            storage.save();
        }

        else => {
            assert (in.body.isEmpty()) throw 0xFFFF;
        }
    }
}

fun onBouncedMessage(in: InMessageBounced) {
}

get fun currentCounter(): int {
    val storage = lazy Storage.load();
    return storage.counter;
}

get fun initialId(): int {
    val storage = lazy Storage.load();
    return storage.id;
}

get fun double(a: int): int {
    return a * 2;
}
"#;

const COUNTER_MESSAGES: &str = r"
struct Storage {
    id: uint32
    counter: uint32
}

fun Storage.load(): Storage {
    return Storage.fromCell(contract.getData());
}

fun Storage.save(self) {
    contract.setData(self.toCell());
}

struct (0x7e8764ef) IncreaseCounter {
    queryId: uint64
    increaseBy: uint32
}

struct (0x3a752f06) ResetCounter {
    queryId: int32
}

struct ResetData {}
";

const MAIN_CODE: &str = r#"
import "../lib/io"
import "../lib/build"
import "../lib/emulation/network"
import "../lib/emulation/testing"
import "../lib/testing/expect"
import "../lib/types/message"
import "../lib/types/out_actions"
import "../lib/fmt"


import "counter_messages"

struct Counter {
    address: address
    init: ContractState
}

fun Counter.fromStorage(storage: Storage): Counter {
    val init = ContractState {
        code: build("counter"),
        data: storage.toCell(),
    };
    val address = AutoDeployAddress { stateInit: init }.calculateAddress();
    return Counter { address, init }
}

fun Counter.sendIncrease(self, from: address, increaseBy: int): SendResultList {
    val msg = createMessage({
        bounce: false,
        value: ton("0.1"),
        dest: self.address,
        body: IncreaseCounter { queryId: 0, increaseBy },
    });
    return net.send(from, msg);
}

fun Counter.sendReset(self, from: address): SendResultList {
    val msg = createMessage({
        bounce: false,
        value: ton("0.1"),
        dest: self.address,
        body: ResetCounter { queryId: 0 },
    });
    return net.send(from, msg);
}

fun Counter.getCounter(self): int {
    return net.runGetMethod(self.address, "currentCounter")
}

fun setupTest(): (Counter, Treasury) {
    val counter = Counter.fromStorage({ id: 0, counter: 0 });

    val deployer = testing.treasury("deployer");
    val msg = createMessage({
        bounce: false,
        value: ton("1.0"),
        dest: {
            stateInit: counter.init,
        },
    });
    val res = net.send(deployer.address, msg);
    expect(res).toHaveSuccessfulDeploy({ to: counter.address });

    return (counter, deployer)
}

get fun `test should run counter script`() {
    val (counter, deployer) = setupTest();

    val counterRes = net.runGetMethod<int>(counter.address, "currentCounter");
    println("Counter: {}", counterRes);

    val info = testing.getAccountState(counter.address)!;
    println("Balance: {:ton}", info.storage.balance.grams);

    val res = counter.sendIncrease(deployer.address, 100);
    println(res);
}

get fun `test should reset counter`() {
    val (counter, deployer) = setupTest();

    val res = counter.sendIncrease(deployer.address, 100);
    expect(res).toHaveSuccessfulTx({ from: deployer.address, to: counter.address });
    expect(counter.getCounter()).toEqual(100);

    val resetRes = counter.sendReset(deployer.address);
    expect(resetRes).toHaveSuccessfulTx({ from: deployer.address, to: counter.address });
    expect(counter.getCounter()).toEqual(0);
}
"#;

fn setup_counter_project(method_name: &str) -> DebugSession {
    let main_code = MAIN_CODE;

    let project = ProjectBuilder::new("counter-test")
        .contract("counter", COUNTER)
        .file("counter_messages", COUNTER_MESSAGES)
        .file("main", main_code)
        .build();

    let builder = DebugBuilder::new("debug-callback")
        .project_ref(project)
        .executable_file("main.tolk");

    builder.method_name(method_name).build()
}

fn setup_jetton_template_project(method_name: &str) -> DebugSession {
    let workspace = ProjectBuilder::new("debug-jetton-template")
        .without_acton_toml()
        .build();
    let project_path = workspace.path().join("jetton-debug");

    workspace
        .acton()
        .arg("new")
        .arg(&project_path.display().to_string())
        .arg("--name")
        .arg("jetton-debug")
        .arg("--template")
        .arg("jetton")
        .arg("--license")
        .arg("MIT")
        .run()
        .success();

    CliDebugBuilder::test(workspace)
        .project_path(project_path)
        .path("tests/wallet-behavior.test.tolk")
        .filter(&format!("^{method_name}$"))
        .build()
}

#[test]
fn test_real_counter_contract_tests() -> anyhow::Result<()> {
    let session = setup_counter_project("test should reset counter");
    let mut client = session.start();

    let result = client.execute(|executor| {
        executor.step_over_times(8)?;
        Ok(())
    })?;

    result.assert_trace_snapshot_matches(
        "integration/snapshots/debugger/counter/test_real_counter_tests.trace.txt",
    );

    Ok(())
}
#[test]
fn test_real_counter_contract_step_in() -> anyhow::Result<()> {
    let session = setup_counter_project("test should run counter script");
    let mut client = session.start();

    let result = client.execute(|executor| {
        executor.step_in_until_terminated(2_000)?;
        Ok(())
    })?;

    result.assert_trace_snapshot_matches(
        "integration/snapshots/debugger/counter/test_real_counter_step_in.trace.txt",
    );

    Ok(())
}

#[test]
fn test_jetton_template_wallet_test_step_over() -> anyhow::Result<()> {
    let session = setup_jetton_template_project("test wallet: owner can send jettons");
    let mut client = session.start();

    let result = client.execute(|executor| {
        executor.step_over_until_terminated(200)?;
        Ok(())
    })?;

    result.assert_trace_snapshot_matches(
        "integration/snapshots/debugger/jetton/test_jetton_template_wallet_test_step_over.trace.txt",
    );

    Ok(())
}

#[test]
fn test_jetton_template_wallet_test_step_in() -> anyhow::Result<()> {
    let session = setup_jetton_template_project("test wallet: owner can send jettons");
    let mut client = session.start();

    let result = client.execute(|executor| {
        executor.step_in_until_terminated(2_000)?;
        Ok(())
    })?;

    result.assert_trace_snapshot_matches(
        "integration/snapshots/debugger/jetton/test_jetton_template_wallet_test_step_in.trace.txt",
    );

    Ok(())
}
