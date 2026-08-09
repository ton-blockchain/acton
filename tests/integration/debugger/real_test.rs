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

const FORWARDER_CONTRACT: &str = r#"import "../counter_messages"

fun onInternalMessage(in: InMessage) {
    if (in.body.isEmpty()) {
        return;
    }

    val msg = lazy TriggerForward.fromSlice(in.body);
    createMessage({
        bounce: false,
        value: ton("0.2"),
        dest: msg.target,
        body: Notify {
            queryId: msg.queryId,
        },
    }).send(SEND_MODE_PAY_FEES_SEPARATELY);
}

fun onBouncedMessage(_: InMessageBounced) {}
"#;

const RECEIVER_CONTRACT: &str = r#"import "../counter_messages"

contract Receiver {
    storage: ReceiverStorage
    incomingMessages: Notify
}

fun onInternalMessage(in: InMessage) {
    if (in.body.isEmpty()) {
        return;
    }

    val msg = lazy Notify.fromSlice(in.body);
    var storage = ReceiverStorage.load();
    storage.received += 1;
    storage.lastQueryId = msg.queryId;
    storage.save();
}

fun onBouncedMessage(_: InMessageBounced) {}

get fun received(): int {
    return ReceiverStorage.load().received;
}

get fun lastQueryId(): int {
    return ReceiverStorage.load().lastQueryId;
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

struct (0x5100f001) TriggerForward {
    queryId: uint64
    target: address
}

struct (0x5100f002) Notify {
    queryId: uint64
}

struct ReceiverStorage {
    received: uint32
    lastQueryId: uint64
}

fun ReceiverStorage.load(): ReceiverStorage {
    return ReceiverStorage.fromCell(contract.getData());
}

fun ReceiverStorage.save(self) {
    contract.setData(self.toCell());
}
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

get fun `test should run counter through tx cursor`() {
    val (counter, deployer) = setupTest();

    val msg = createMessage({
        bounce: false,
        value: ton("0.1"),
        dest: counter.address,
        body: IncreaseCounter { queryId: 0, increaseBy: 7 },
    });

    val cursor = testing.createTraceIterationCursor(deployer.address, msg);
    val res = cursor.executeN(1);

    expect(res).toHaveSuccessfulTx({ from: deployer.address, to: counter.address });
    expect(counter.getCounter()).toEqual(7);
}

struct Forwarder {
    address: address
    init: ContractState
}

fun Forwarder.fromEmpty(): Forwarder {
    val init = ContractState {
        code: build("forwarder"),
        data: createEmptyCell(),
    };
    val address = AutoDeployAddress { stateInit: init }.calculateAddress();
    return Forwarder { address, init };
}

fun Forwarder.deploy(self, from: address, config: SendParams = {}): SendResultList {
    return net.send(from, createMessage({
        bounce: config.bounce,
        value: config.value,
        dest: { stateInit: self.init },
    }));
}

fun Forwarder.createTriggerMessage(
    self,
    target: address,
    queryId: uint64,
    config: SendParams = {},
): OutMessage {
    return createMessage({
        bounce: config.bounce,
        value: config.value,
        dest: self.address,
        body: TriggerForward { queryId, target },
    });
}

struct Receiver {
    address: address
    init: ContractState
}

fun Receiver.fromStorage(storage: ReceiverStorage): Receiver {
    val init = ContractState {
        code: build("receiver"),
        data: storage.toCell(),
    };
    val address = AutoDeployAddress { stateInit: init }.calculateAddress();
    return Receiver { address, init };
}

fun Receiver.deploy(self, from: address, config: SendParams = {}): SendResultList {
    return net.send(from, createMessage({
        bounce: config.bounce,
        value: config.value,
        dest: { stateInit: self.init },
    }));
}

fun Receiver.received(self): int {
    return net.runGetMethod(self.address, "received");
}

fun Receiver.lastQueryId(self): int {
    return net.runGetMethod(self.address, "lastQueryId");
}

get fun `test txcursor can split and resume a forwarded chain`() {
    val deployer = testing.treasury("deployer");
    val forwarder = Forwarder.fromEmpty();
    val receiver = Receiver.fromStorage({
        received: 0,
        lastQueryId: 0,
    });

    expect(forwarder.deploy(deployer.address, { value: ton("1") })).toHaveSuccessfulDeploy({
        to: forwarder.address,
    });
    expect(receiver.deploy(deployer.address, { value: ton("1") })).toHaveSuccessfulDeploy({
        to: receiver.address,
    });

    val trigger = forwarder.createTriggerMessage(receiver.address, 42, { value: ton("0.5") });
    val cursor = testing.createTraceIterationCursor(deployer.address, trigger);

    val firstHop = cursor.executeN(1);
    expect(firstHop).toHaveSuccessfulTx<TriggerForward>({
        from: deployer.address,
        to: forwarder.address,
    });
    expect(receiver.received()).toEqual(0);
    expect(cursor.isDone()).toEqual(false);

    val receiverHop = cursor.executeTill<Notify>({
        from: forwarder.address,
        to: receiver.address,
    });
    expect(receiverHop).toHaveSuccessfulTx<Notify>({
        from: forwarder.address,
        to: receiver.address,
    });
    expect(receiver.received()).toEqual(1);
    expect(receiver.lastQueryId()).toEqual(42);
    expect(cursor.isDone()).toEqual(true);

    val secondTrigger = forwarder.createTriggerMessage(receiver.address, 43, { value: ton("0.5") });
    val secondCursor = testing.createTraceIterationCursor(deployer.address, secondTrigger);
    val allRemaining = secondCursor.executeAllRemaining();

    expect(allRemaining).toHaveSuccessfulTx<TriggerForward>({
        from: deployer.address,
        to: forwarder.address,
    });
    expect(allRemaining).toHaveSuccessfulTx<Notify>({
        from: forwarder.address,
        to: receiver.address,
    });
    expect(receiver.received()).toEqual(2);
    expect(receiver.lastQueryId()).toEqual(43);
    expect(secondCursor.isDone()).toEqual(true);
}
"#;

fn setup_counter_project(method_name: &str) -> DebugSession {
    let main_code = MAIN_CODE;

    let project = ProjectBuilder::new("counter-test")
        .contract("counter", COUNTER)
        .contract("forwarder", FORWARDER_CONTRACT)
        .contract("receiver", RECEIVER_CONTRACT)
        .file("counter_messages", COUNTER_MESSAGES)
        .file("main", main_code)
        .build();

    let builder = DebugBuilder::new("debug-callback")
        .project_ref(project)
        .executable_file("main.tolk")
        .without_outer_frame_local_snapshots();

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
fn test_real_counter_contract_tx_cursor_step_in() -> anyhow::Result<()> {
    let session = setup_counter_project("test should run counter through tx cursor");
    let mut client = session.start();

    let result = client.execute(|executor| {
        executor.step_in_until_terminated(2_000)?;
        Ok(())
    })?;

    result.assert_trace_snapshot_matches(
        "integration/snapshots/debugger/counter/test_real_counter_tx_cursor_step_in.trace.txt",
    );

    Ok(())
}

#[test]
fn test_real_counter_contract_tx_cursor_forwarded_chain_step_in() -> anyhow::Result<()> {
    let session = setup_counter_project("test txcursor can split and resume a forwarded chain");
    let mut client = session.start();

    let result = client.execute(|executor| {
        executor.step_in_until_terminated(4_000)?;
        Ok(())
    })?;

    result.assert_trace_snapshot_matches(
        "integration/snapshots/debugger/counter/test_real_counter_tx_cursor_forwarded_chain_step_in.trace.txt",
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
