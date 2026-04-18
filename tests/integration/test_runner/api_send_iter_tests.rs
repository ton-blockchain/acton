use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const SNAPSHOT_DIR: &str = "integration/snapshots/test-runner/api_send_iter";

const ITER_MESSAGES: &str = r"
struct (0x3100f001) TriggerForward {
    queryId: uint64
    target: address
}

struct (0x3100f002) Notify {
    queryId: uint64
}

struct (0x3100f003) TriggerRoute {
    queryId: uint64
    relay: address
    sink: address
}

struct (0x3100f004) Relay {
    queryId: uint64
    sink: address
}

struct (0x3100f005) Touch {
    queryId: uint64
}

struct (0x3100f006) Begin {
    queryId: uint64
}

struct (0x3100f007) Finish {
    queryId: uint64
}

struct (0x3100f008) Attack {
    queryId: uint64
}

struct (0x3100f009) ExternalNotice {
    queryId: uint64
}
";

const FORWARDER_CONTRACT: &str = r#"
import "messages"

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

const RECEIVER_CONTRACT: &str = r#"
import "messages"

struct Storage {
    received: uint32
}

fun loadStorage() {
    val data = contract.getData();
    val slice = data.beginParse();
    if (slice.remainingBitsCount() == 0 && slice.remainingRefsCount() == 0) {
        return Storage { received: 0 };
    }
    return Storage.fromCell(data);
}

fun saveStorage(data: Storage) {
    contract.setData(data.toCell());
}

fun onInternalMessage(in: InMessage) {
    if (in.body.isEmpty()) {
        return;
    }

    val _msg = lazy Notify.fromSlice(in.body);
    var storage = loadStorage();
    storage.received = storage.received + 1;
    saveStorage(storage);
}

fun onBouncedMessage(_: InMessageBounced) {}

get fun received(): int {
    return loadStorage().received;
}
"#;

const EXTERNAL_FORWARDER_CONTRACT: &str = r#"
import "messages"

fun onInternalMessage(in: InMessage) {
    if (in.body.isEmpty()) {
        return;
    }

    val msg = lazy TriggerForward.fromSlice(in.body);

    createExternalLogMessage({
        dest: createAddressNone(),
        body: ExternalNotice {
            queryId: msg.queryId,
        },
    }).send(SEND_MODE_REGULAR);

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

const LIBRARY_CHILD_CONTRACT: &str = r"
fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
";

const LIBRARY_SPAWNER_CONTRACT: &str = r#"
import "../gen/lib.code.tolk"

fun onInternalMessage(in: InMessage) {
    if (in.body.isEmpty()) {
        return;
    }

    val childInit = ContractState {
        code: libCompiledCode(),
        data: createEmptyCell(),
    };

    createMessage({
        bounce: false,
        value: ton("0.2"),
        dest: {
            stateInit: childInit,
        },
        body: beginCell().storeUint(777, 32).endCell(),
    }).send(SEND_MODE_PAY_FEES_SEPARATELY);
}

fun onBouncedMessage(_: InMessageBounced) {}
"#;

const ROUTER_CONTRACT: &str = r#"
import "messages"

fun onInternalMessage(in: InMessage) {
    if (in.body.isEmpty()) {
        return;
    }

    val msg = lazy TriggerRoute.fromSlice(in.body);
    createMessage({
        bounce: false,
        value: ton("0.2"),
        dest: msg.relay,
        body: Relay {
            queryId: msg.queryId,
            sink: msg.sink,
        },
    }).send(SEND_MODE_PAY_FEES_SEPARATELY);
}

fun onBouncedMessage(_: InMessageBounced) {}
"#;

const RELAY_CONTRACT: &str = r#"
import "messages"

fun onInternalMessage(in: InMessage) {
    if (in.body.isEmpty()) {
        return;
    }

    val msg = lazy Relay.fromSlice(in.body);
    createMessage({
        bounce: false,
        value: ton("0.1"),
        dest: msg.sink,
        body: Touch {
            queryId: msg.queryId,
        },
    }).send(SEND_MODE_PAY_FEES_SEPARATELY);
}

fun onBouncedMessage(_: InMessageBounced) {}
"#;

const SINK_CONTRACT: &str = r#"
import "messages"

struct Storage {
    touches: uint32
}

fun loadStorage() {
    val data = contract.getData();
    val slice = data.beginParse();
    if (slice.remainingBitsCount() == 0 && slice.remainingRefsCount() == 0) {
        return Storage { touches: 0 };
    }
    return Storage.fromCell(data);
}

fun saveStorage(data: Storage) {
    contract.setData(data.toCell());
}

fun onInternalMessage(in: InMessage) {
    if (in.body.isEmpty()) {
        return;
    }

    val _msg = lazy Touch.fromSlice(in.body);
    var storage = loadStorage();
    storage.touches = storage.touches + 1;
    saveStorage(storage);
}

fun onBouncedMessage(_: InMessageBounced) {}

get fun touches(): int {
    return loadStorage().touches;
}
"#;

const RACE_CONTRACT: &str = r#"
import "messages"

type AllowedRaceMessage = Begin | Finish | Attack

struct Storage {
    stage: uint32
    attacked: uint32
}

fun loadStorage() {
    val data = contract.getData();
    val slice = data.beginParse();
    if (slice.remainingBitsCount() == 0 && slice.remainingRefsCount() == 0) {
        return Storage { stage: 0, attacked: 0 };
    }
    return Storage.fromCell(data);
}

fun saveStorage(data: Storage) {
    contract.setData(data.toCell());
}

fun onInternalMessage(in: InMessage) {
    val msg = lazy AllowedRaceMessage.fromSlice(in.body);
    var storage = loadStorage();

    match (msg) {
        Begin => {
            storage.stage = 1;
            saveStorage(storage);

            createMessage({
                bounce: false,
                value: ton("0.1"),
                dest: contract.getAddress(),
                body: Finish {
                    queryId: msg.queryId,
                },
            }).send(SEND_MODE_PAY_FEES_SEPARATELY);
        }

        Finish => {
            if (storage.stage != 1) {
                throw 901;
            }
            storage.stage = 2;
            saveStorage(storage);
        }

        Attack => {
            if (storage.stage == 1) {
                storage.attacked = storage.attacked + 1;
            } else {
                storage.attacked = storage.attacked + 100;
            }
            saveStorage(storage);
        }

        else => {
            assert (in.body.isEmpty()) throw 902;
        }
    }
}

fun onBouncedMessage(_: InMessageBounced) {}

get fun stage(): int {
    return loadStorage().stage;
}

get fun attacked(): int {
    return loadStorage().attacked;
}
"#;

const TEST_IMPORTS: &str = r#"
import "../../lib/build"
import "../../lib/emulation/network"
import "../../lib/emulation/testing"
import "../../lib/testing/expect"
import "../../lib/types/big_array"
import "../../lib/types/message"
import "../contracts/messages"
"#;

fn build_send_iter_project(name: &str) -> ProjectBuilder {
    ProjectBuilder::new(name)
        .file("contracts/messages", ITER_MESSAGES)
        .contract("external_forwarder", EXTERNAL_FORWARDER_CONTRACT)
        .contract("forwarder", FORWARDER_CONTRACT)
        .contract("receiver", RECEIVER_CONTRACT)
        .contract("router", ROUTER_CONTRACT)
        .contract("relay", RELAY_CONTRACT)
        .contract("sink", SINK_CONTRACT)
        .contract("race", RACE_CONTRACT)
}

fn run_send_iter_success(project_name: &str, test_body: &str, snapshot_name: &str) {
    build_send_iter_project(project_name)
        .test_file("test", &format!("{TEST_IMPORTS}\n{test_body}\n"))
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(&format!("{SNAPSHOT_DIR}/{snapshot_name}.stdout.txt"));
}

fn run_send_iter_failure(project_name: &str, test_body: &str, snapshot_name: &str) {
    build_send_iter_project(project_name)
        .test_file("test", &format!("{TEST_IMPORTS}\n{test_body}\n"))
        .build()
        .acton()
        .test()
        .run()
        .failure()
        .assert_failed(1)
        .assert_snapshot_matches(&format!("{SNAPSHOT_DIR}/{snapshot_name}.stdout.txt"));
}

#[test]
fn send_iter_execute_n_processes_first_hop_and_execute_from_drains_rest() {
    run_send_iter_success(
        "n-lib-api-send-iter-execute-n",
        r#"
get fun `test send iter execute n and from`() {
    val sender = testing.treasury("sender");

    val forwarderInit = ContractState {
        code: build("forwarder"),
        data: createEmptyCell(),
    };
    val forwarderAddress = AutoDeployAddress { stateInit: forwarderInit }.calculateAddress();

    val receiverInit = ContractState {
        code: build("receiver"),
        data: createEmptyCell(),
    };
    val receiverAddress = AutoDeployAddress { stateInit: receiverInit }.calculateAddress();

    expect(net.send(sender.address, createMessage({
        bounce: false,
        value: ton("1"),
        dest: { stateInit: forwarderInit },
    }))).toHaveSuccessfulDeploy({ to: forwarderAddress });

    expect(net.send(sender.address, createMessage({
        bounce: false,
        value: ton("1"),
        dest: { stateInit: receiverInit },
    }))).toHaveSuccessfulDeploy({ to: receiverAddress });

    val trigger = createMessage({
        bounce: false,
        value: ton("0.5"),
        dest: forwarderAddress,
        body: TriggerForward {
            queryId: 7,
            target: receiverAddress,
        },
    });

    val iter = testing.createTraceIterationCursor(sender.address, trigger);
    expect(iter.isDone()).toBeFalse();

    val first = iter.executeN(1);
    expect(first.size()).toEqual(1);
    expect(first).toHaveSuccessfulTx<TriggerForward>({
        from: sender.address,
        to: forwarderAddress,
    });
    expect(first.at(0).childTxs.size()).toEqual(0);
    expect(net.runGetMethod<int>(receiverAddress, "received")).toEqual(0);
    expect(iter.isDone()).toBeFalse();

    val rest = iter.executeAllRemaining();
    expect(rest.size()).toEqual(1);
    expect(rest).toHaveSuccessfulTx<Notify>({
        from: forwarderAddress,
        to: receiverAddress,
    });
    expect(rest.at(0).parentLt).toEqual(first.at(0).tx.load().lt);
    expect(net.runGetMethod<int>(receiverAddress, "received")).toEqual(1);
    expect(iter.isDone()).toBeTrue();

    val discarded = testing.createTraceIterationCursor(sender.address, trigger);
    discarded.close();
    expect(discarded.isDone()).toBeTrue();
    expect(discarded.executeAllRemaining()).toBeEmpty();
    expect(net.runGetMethod<int>(receiverAddress, "received")).toEqual(1);
}
"#,
        "send_iter_execute_n_processes_first_hop_and_execute_from_drains_rest",
    );
}

#[test]
fn send_iter_execute_till_supports_predicate_search_params() {
    run_send_iter_success(
        "n-lib-api-send-iter-execute-till-predicate-search-params",
        r#"
import "../../lib/io"

get fun `test send iter execute till predicate search params`() {
    val sender = testing.treasury("sender");

    val forwarderInit = ContractState {
        code: build("forwarder"),
        data: createEmptyCell(),
    };
    val forwarderAddress = AutoDeployAddress { stateInit: forwarderInit }.calculateAddress();

    val receiverInit = ContractState {
        code: build("receiver"),
        data: createEmptyCell(),
    };
    val receiverAddress = AutoDeployAddress { stateInit: receiverInit }.calculateAddress();

    expect(net.send(sender.address, createMessage({
        bounce: false,
        value: ton("1"),
        dest: { stateInit: forwarderInit },
    }))).toHaveSuccessfulDeploy({ to: forwarderAddress });

    expect(net.send(sender.address, createMessage({
        bounce: false,
        value: ton("1"),
        dest: { stateInit: receiverInit },
    }))).toHaveSuccessfulDeploy({ to: receiverAddress });

    val iter = testing.createTraceIterationCursor(sender.address, createMessage({
        bounce: false,
        value: ton("0.5"),
        dest: forwarderAddress,
        body: TriggerForward {
            queryId: 77,
            target: receiverAddress,
        },
    }));

    val segment = iter.executeTill({
        to: fun(addr: address): bool {
            println("executeTill.to={}", addr);
            return addr == receiverAddress;
        },
        from: fun(addr: address): bool {
            println("executeTill.from={}", addr);
            return addr == forwarderAddress;
        },
        opcode: fun(op: uint32): bool {
            println("executeTill.opcode=0x{:x}", op);
            return op == Notify.__getDeclaredPackPrefix();
        },
        success: fun(ok: bool): bool {
            println("executeTill.success={}", ok);
            return ok;
        },
    });

    expect(segment).toHaveLength(2);
    expect(segment).toHaveSuccessfulTx<Notify>({
        from: forwarderAddress,
        to: receiverAddress,
    });
    expect(iter.isDone()).toBeTrue();
    expect(net.runGetMethod<int>(receiverAddress, "received")).toEqual(1);
}
"#,
        "send_iter_execute_till_supports_predicate_search_params",
    );
}

#[test]
fn send_iter_execute_till_stops_at_matching_transaction_and_preserves_tail() {
    run_send_iter_success(
        "n-lib-api-send-iter-execute-till",
        r#"
get fun `test send iter execute till`() {
    val sender = testing.treasury("sender");

    val routerInit = ContractState {
        code: build("router"),
        data: createEmptyCell(),
    };
    val routerAddress = AutoDeployAddress { stateInit: routerInit }.calculateAddress();

    val relayInit = ContractState {
        code: build("relay"),
        data: createEmptyCell(),
    };
    val relayAddress = AutoDeployAddress { stateInit: relayInit }.calculateAddress();

    val sinkInit = ContractState {
        code: build("sink"),
        data: createEmptyCell(),
    };
    val sinkAddress = AutoDeployAddress { stateInit: sinkInit }.calculateAddress();

    expect(net.send(sender.address, createMessage({
        bounce: false,
        value: ton("1"),
        dest: { stateInit: routerInit },
    }))).toHaveSuccessfulDeploy({ to: routerAddress });
    expect(net.send(sender.address, createMessage({
        bounce: false,
        value: ton("1"),
        dest: { stateInit: relayInit },
    }))).toHaveSuccessfulDeploy({ to: relayAddress });
    expect(net.send(sender.address, createMessage({
        bounce: false,
        value: ton("1"),
        dest: { stateInit: sinkInit },
    }))).toHaveSuccessfulDeploy({ to: sinkAddress });

    val trigger = createMessage({
        bounce: false,
        value: ton("0.5"),
        dest: routerAddress,
        body: TriggerRoute {
            queryId: 17,
            relay: relayAddress,
            sink: sinkAddress,
        },
    });

    val iter = testing.createTraceIterationCursor(sender.address, trigger);
    val untilRelay = iter.executeTill<Relay>({
        from: routerAddress,
        to: relayAddress,
    });

    expect(untilRelay.size()).toEqual(2);
    expect(untilRelay).toHaveSuccessfulTx<TriggerRoute>({
        from: sender.address,
        to: routerAddress,
    });
    expect(untilRelay).toHaveSuccessfulTx<Relay>({
        from: routerAddress,
        to: relayAddress,
    });
    expect(net.runGetMethod<int>(sinkAddress, "touches")).toEqual(0);
    expect(iter.isDone()).toBeFalse();

    val tail = iter.executeAllRemaining();
    expect(tail.size()).toEqual(1);
    expect(tail).toHaveSuccessfulTx<Touch>({
        from: relayAddress,
        to: sinkAddress,
    });
    expect(net.runGetMethod<int>(sinkAddress, "touches")).toEqual(1);
    expect(iter.isDone()).toBeTrue();
}
"#,
        "send_iter_execute_till_stops_at_matching_transaction_and_preserves_tail",
    );
}

#[test]
fn send_iter_allows_interleaving_multiple_cursors_against_shared_world_state() {
    run_send_iter_success(
        "n-lib-api-send-iter-interleaving",
        r#"
get fun `test send iter interleaving`() {
    val sender = testing.treasury("sender");
    val attacker = testing.treasury("attacker");

    val raceInit = ContractState {
        code: build("race"),
        data: createEmptyCell(),
    };
    val raceAddress = AutoDeployAddress { stateInit: raceInit }.calculateAddress();

    expect(net.send(sender.address, createMessage({
        bounce: false,
        value: ton("1"),
        dest: { stateInit: raceInit },
    }))).toHaveSuccessfulDeploy({ to: raceAddress });

    val beginIter = testing.createTraceIterationCursor(sender.address, createMessage({
        bounce: false,
        value: ton("0.5"),
        dest: raceAddress,
        body: Begin {
            queryId: 1,
        },
    }));
    val attackIter = testing.createTraceIterationCursor(attacker.address, createMessage({
        bounce: false,
        value: ton("0.5"),
        dest: raceAddress,
        body: Attack {
            queryId: 2,
        },
    }));

    val beginFirst = beginIter.executeN(1);
    expect(beginFirst.size()).toEqual(1);
    expect(beginFirst).toHaveSuccessfulTx<Begin>({
        from: sender.address,
        to: raceAddress,
    });
    expect(net.runGetMethod<int>(raceAddress, "stage")).toEqual(1);

    val attackAll = attackIter.executeAllRemaining();
    expect(attackAll.size()).toEqual(1);
    expect(attackAll).toHaveSuccessfulTx<Attack>({
        from: attacker.address,
        to: raceAddress,
    });
    expect(net.runGetMethod<int>(raceAddress, "attacked")).toEqual(1);

    val beginTail = beginIter.executeAllRemaining();
    expect(beginTail.size()).toEqual(1);
    expect(beginTail).toHaveSuccessfulTx<Finish>({
        from: raceAddress,
        to: raceAddress,
    });
    expect(beginTail.at(0).parentLt).toEqual(beginFirst.at(0).tx.load().lt);
    expect(net.runGetMethod<int>(raceAddress, "stage")).toEqual(2);
}
"#,
        "send_iter_allows_interleaving_multiple_cursors_against_shared_world_state",
    );
}

#[test]
fn send_iter_execute_n_zero_is_noop_and_overshoot_backfills_relationships() {
    run_send_iter_success(
        "n-lib-api-send-iter-zero-and-overshoot",
        r#"
get fun `test send iter zero and overshoot`() {
    val sender = testing.treasury("sender");

    val forwarderInit = ContractState {
        code: build("forwarder"),
        data: createEmptyCell(),
    };
    val forwarderAddress = AutoDeployAddress { stateInit: forwarderInit }.calculateAddress();

    val receiverInit = ContractState {
        code: build("receiver"),
        data: createEmptyCell(),
    };
    val receiverAddress = AutoDeployAddress { stateInit: receiverInit }.calculateAddress();

    expect(net.send(sender.address, createMessage({
        bounce: false,
        value: ton("1"),
        dest: { stateInit: forwarderInit },
    }))).toHaveSuccessfulDeploy({ to: forwarderAddress });

    expect(net.send(sender.address, createMessage({
        bounce: false,
        value: ton("1"),
        dest: { stateInit: receiverInit },
    }))).toHaveSuccessfulDeploy({ to: receiverAddress });

    val iter = testing.createTraceIterationCursor(sender.address, createMessage({
        bounce: false,
        value: ton("0.5"),
        dest: forwarderAddress,
        body: TriggerForward {
            queryId: 77,
            target: receiverAddress,
        },
    }));

    val zero = iter.executeN(0);
    expect(zero).toBeEmpty();
    expect(iter.isDone()).toBeFalse();
    expect(net.runGetMethod<int>(receiverAddress, "received")).toEqual(0);

    val all = iter.executeN(10);
    expect(all.size()).toEqual(2);
    expect(all.at(0).childTxs.size()).toEqual(1);
    expect(all.at(0).childTxs.get(0)).toEqual(all.at(1).tx.load().lt);
    expect(all.at(1).parentLt).toEqual(all.at(0).tx.load().lt);
    expect(iter.isDone()).toBeTrue();
    expect(iter.executeN(1)).toBeEmpty();
    expect(net.runGetMethod<int>(receiverAddress, "received")).toEqual(1);
}
"#,
        "send_iter_execute_n_zero_is_noop_and_overshoot_backfills_relationships",
    );
}

#[test]
fn send_iter_collects_external_messages_and_keeps_internal_tail_queued() {
    run_send_iter_success(
        "n-lib-api-send-iter-externals",
        r#"
get fun `test send iter collects externals`() {
    val sender = testing.treasury("sender");

    val emitterInit = ContractState {
        code: build("external_forwarder"),
        data: createEmptyCell(),
    };
    val emitterAddress = AutoDeployAddress { stateInit: emitterInit }.calculateAddress();

    val receiverInit = ContractState {
        code: build("receiver"),
        data: createEmptyCell(),
    };
    val receiverAddress = AutoDeployAddress { stateInit: receiverInit }.calculateAddress();

    expect(net.send(sender.address, createMessage({
        bounce: false,
        value: ton("1"),
        dest: { stateInit: emitterInit },
    }))).toHaveSuccessfulDeploy({ to: emitterAddress });

    expect(net.send(sender.address, createMessage({
        bounce: false,
        value: ton("1"),
        dest: { stateInit: receiverInit },
    }))).toHaveSuccessfulDeploy({ to: receiverAddress });

    val iter = testing.createTraceIterationCursor(sender.address, createMessage({
        bounce: false,
        value: ton("0.5"),
        dest: emitterAddress,
        body: TriggerForward {
            queryId: 55,
            target: receiverAddress,
        },
    }));

    val first = iter.executeN(1);
    expect(first).toHaveLength(1);
    expect(first.at(0).externals).toHaveLength(1);

    val external = first.at(0).externals.at<ExternalNotice>(0);
    expect(external.info.src).toEqual(emitterAddress);
    expect(external.info.dest).toEqual(createAddressNone());
    expect(external.loadBody()).toEqual(ExternalNotice { queryId: 55 });

    expect(net.runGetMethod<int>(receiverAddress, "received")).toEqual(0);
    expect(iter.isDone()).toBeFalse();

    val tail = iter.executeAllRemaining();
    expect(tail).toHaveLength(1);
    expect(tail).toHaveSuccessfulTx<Notify>({
        from: emitterAddress,
        to: receiverAddress,
    });
    expect(net.runGetMethod<int>(receiverAddress, "received")).toEqual(1);
}
"#,
        "send_iter_collects_external_messages_and_keeps_internal_tail_queued",
    );
}

#[test]
fn send_iter_uses_live_registered_libraries_for_resumed_steps() {
    ProjectBuilder::new("n-lib-api-send-iter-live-libraries")
        .contract("lib", LIBRARY_CHILD_CONTRACT)
        .contract_with_detailed_deps(
            "spawner",
            LIBRARY_SPAWNER_CONTRACT,
            vec![("lib", Some("library_ref"), None, None)],
        )
        .test_file(
            "test",
            r#"
import "../../lib/build"
import "../../lib/emulation/network"
import "../../lib/emulation/testing"
import "../../lib/testing/expect"
import "../gen/lib.code.tolk"

get fun `test send iter live libraries`() {
    val sender = testing.treasury("sender");

    val spawnerInit = ContractState {
        code: build("spawner"),
        data: createEmptyCell(),
    };
    val spawnerAddress = AutoDeployAddress { stateInit: spawnerInit }.calculateAddress();

    val childInit = ContractState {
        code: libCompiledCode(),
        data: createEmptyCell(),
    };
    val childAddress = AutoDeployAddress { stateInit: childInit }.calculateAddress();

    expect(net.send(sender.address, createMessage({
        bounce: false,
        value: ton("1"),
        dest: { stateInit: spawnerInit },
    }))).toHaveSuccessfulDeploy({ to: spawnerAddress });

    val iter = testing.createTraceIterationCursor(sender.address, createMessage({
        bounce: false,
        value: ton("0.5"),
        dest: spawnerAddress,
        body: beginCell().storeUint(1, 32).endCell(),
    }));

    val first = iter.executeN(1);
    expect(first).toHaveLength(1);
    expect(first).toHaveSuccessfulTx({
        from: sender.address,
        to: spawnerAddress,
    });

    testing.registerLibrary(build("lib"));

    val tail = iter.executeAllRemaining();
    expect(tail).toHaveLength(1);
    expect(tail).toHaveSuccessfulDeploy({ to: childAddress });
    expect(iter.isDone()).toBeTrue();
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
            "integration/snapshots/test-runner/api_send_iter/send_iter_uses_live_registered_libraries_for_resumed_steps.stdout.txt",
        );
}

#[test]
fn send_iter_close_after_partial_execution_discards_tail_and_bogus_cursor_is_empty() {
    run_send_iter_success(
        "n-lib-api-send-iter-close-and-bogus",
        r#"
get fun `test send iter close after partial execution`() {
    val sender = testing.treasury("sender");

    val forwarderInit = ContractState {
        code: build("forwarder"),
        data: createEmptyCell(),
    };
    val forwarderAddress = AutoDeployAddress { stateInit: forwarderInit }.calculateAddress();

    val receiverInit = ContractState {
        code: build("receiver"),
        data: createEmptyCell(),
    };
    val receiverAddress = AutoDeployAddress { stateInit: receiverInit }.calculateAddress();

    expect(net.send(sender.address, createMessage({
        bounce: false,
        value: ton("1"),
        dest: { stateInit: forwarderInit },
    }))).toHaveSuccessfulDeploy({ to: forwarderAddress });

    expect(net.send(sender.address, createMessage({
        bounce: false,
        value: ton("1"),
        dest: { stateInit: receiverInit },
    }))).toHaveSuccessfulDeploy({ to: receiverAddress });

    val iter = testing.createTraceIterationCursor(sender.address, createMessage({
        bounce: false,
        value: ton("0.5"),
        dest: forwarderAddress,
        body: TriggerForward {
            queryId: 88,
            target: receiverAddress,
        },
    }));

    val first = iter.executeN(1);
    expect(first.size()).toEqual(1);
    expect(net.runGetMethod<int>(receiverAddress, "received")).toEqual(0);
    iter.close();
    expect(iter.isDone()).toBeTrue();
    expect(iter.executeAllRemaining()).toBeEmpty();
    expect(net.runGetMethod<int>(receiverAddress, "received")).toEqual(0);

    val bogus = TxCursor { id: 999999 };
    expect(bogus.isDone()).toBeTrue();
    expect(bogus.executeN(3)).toBeEmpty();
    expect(bogus.executeAllRemaining()).toBeEmpty();
}
"#,
        "send_iter_close_after_partial_execution_discards_tail_and_bogus_cursor_is_empty",
    );
}

#[test]
fn send_iter_rejects_broadcast_mode_before_cursor_creation() {
    run_send_iter_failure(
        "n-lib-api-send-iter-broadcast-reject",
        r#"
get fun `test send iter rejects broadcast mode`() {
    val sender = testing.treasury("sender");
    val receiverInit = ContractState {
        code: build("receiver"),
        data: createEmptyCell(),
    };
    val receiverAddress = AutoDeployAddress { stateInit: receiverInit }.calculateAddress();

    net.enableBroadcast();

    testing.createTraceIterationCursor(sender.address, createMessage({
        bounce: false,
        value: ton("0.5"),
        dest: receiverAddress,
    }));
}
"#,
        "send_iter_rejects_broadcast_mode_before_cursor_creation",
    );
}

#[test]
fn send_iter_invalid_message_reports_parse_error_before_execution() {
    run_send_iter_failure(
        "n-lib-api-send-iter-invalid-message",
        r#"
get fun `test send iter invalid message`() {
    val sender = testing.treasury("sender");
    val invalidAddress = AutoDeployAddress {
        stateInit: beginCell()
            .storeBool(false)
            .endCell(),
    };

    val iter = testing.createTraceIterationCursor(sender.address, createMessage({
        bounce: false,
        value: ton("0.5"),
        dest: invalidAddress,
    }));

    iter.executeN(1);
}
"#,
        "send_iter_invalid_message_reports_parse_error_before_execution",
    );
}

#[test]
fn send_iter_execute_till_without_match_fails_after_queue_exhaustion() {
    run_send_iter_failure(
        "n-lib-api-send-iter-execute-till-miss",
        r#"
get fun `test send iter execute till miss`() {
    val sender = testing.treasury("sender");

    val forwarderInit = ContractState {
        code: build("forwarder"),
        data: createEmptyCell(),
    };
    val forwarderAddress = AutoDeployAddress { stateInit: forwarderInit }.calculateAddress();

    val receiverInit = ContractState {
        code: build("receiver"),
        data: createEmptyCell(),
    };
    val receiverAddress = AutoDeployAddress { stateInit: receiverInit }.calculateAddress();

    expect(net.send(sender.address, createMessage({
        bounce: false,
        value: ton("1"),
        dest: { stateInit: forwarderInit },
    }))).toHaveSuccessfulDeploy({ to: forwarderAddress });

    expect(net.send(sender.address, createMessage({
        bounce: false,
        value: ton("1"),
        dest: { stateInit: receiverInit },
    }))).toHaveSuccessfulDeploy({ to: receiverAddress });

    val iter = testing.createTraceIterationCursor(sender.address, createMessage({
        bounce: false,
        value: ton("0.5"),
        dest: forwarderAddress,
        body: TriggerForward {
            queryId: 99,
            target: receiverAddress,
        },
    }));

    iter.executeTill<Touch>();
}
"#,
        "send_iter_execute_till_without_match_fails_after_queue_exhaustion",
    );
}
