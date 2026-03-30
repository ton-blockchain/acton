import "@local-acton/emulation/network"
import "@local-acton/build/build"
import "@local-acton/ffi/ffi"
import "@local-acton/ffi/reverse-ffi"
import "@local-acton/testing/expect"
import "@local-acton/testing/transaction_expect"
import "contracts/types"

fun x2(x: int): int {
    return x * 2;
}

get fun `test executeInSeparateLibemulator`() {
    val res = ffi.__executeInSeparateLibemulator(x2, 300);
    expect(res).toEqual(600);
}

struct Counter {
    address: address
    init: ContractState
}

fun Counter.fromStorage(storage: Storage): Counter {
    val init = ContractState {
        code: build("Counter", "tests/acton-stdlib/contracts/counter.tolk"),
        data: storage.toCell(),
    };
    val address = AutoDeployAddress { stateInit: init }.calculateAddress();
    return Counter { address, init }
}

fun setupTest(): (Counter, Treasury) {
    val counter = Counter.fromStorage({ id: 0, counter: 0 });
    val deployer = net.treasury("deployer");
    val msg = createMessage({
        bounce: false,
        value: ton("1.0"),
        dest: { stateInit: counter.init },
    });
    net.send(deployer.address, msg);
    return (counter, deployer)
}

fun Counter.sendIncrease(self, from: address, increaseBy: int): SendResultList {
    val msg = createMessage({
        bounce: false,
        value: ton("0.1"),
        dest: self.address,
        body: IncreaseCounter { increaseBy },
    });
    return net.send(from, msg);
}

get fun `test toHaveSuccessfulTx`() {
    val (counter, deployer) = setupTest();
    val res = counter.sendIncrease(deployer.address, 42);
    expect(res).toHaveSuccessfulTx({ to: counter.address, from: deployer.address });
}

get fun `test toHaveSuccessfulDeploy`() {
    val counter = Counter.fromStorage({ id: 0, counter: 0 });
    val deployer = net.treasury("deployer");
    val msg = createMessage({
        bounce: false,
        value: ton("1.0"),
        dest: { stateInit: counter.init },
    });
    val res = net.send(deployer.address, msg);
    expect(res).toHaveSuccessfulDeploy({ to: counter.address });
}

get fun `test toHaveTx`() {
    val (counter, deployer) = setupTest();
    val res = counter.sendIncrease(deployer.address, 42);
    expect(res).toHaveTx({ to: counter.address });
}

get fun `test toNotHaveTx`() {
    val (counter, deployer) = setupTest();
    val res = counter.sendIncrease(deployer.address, 42);
    val other = net.treasury("other");
    expect(res).toNotHaveTx({ from: other.address, to: counter.address });
}

get fun `test toHaveFailedTx`() {
    val (counter, deployer) = setupTest();
    val msg = createMessage({
        bounce: false,
        value: ton("0.1"),
        dest: counter.address,
        body: beginCell().storeUint(0xDEADBEEF, 32).endCell(),
    });
    val res = net.send(deployer.address, msg);
    expect(res).toHaveFailedTx({ to: counter.address, exitCode: 0xFFFF });
}

get fun `test toHaveAllSuccessfulTxs`() {
    val (counter, deployer) = setupTest();
    val res = counter.sendIncrease(deployer.address, 42);
    expect(res).toHaveAllSuccessfulTxs();
}

fun isNotZero(code: int32): bool { return code != 0; }
fun isZero(code: int32): bool { return code == 0; }
fun isFalse(s: bool): bool { return !s; }
fun isTrue(s: bool): bool { return s; }
fun alwaysFalseAddr(addr: address): bool { addr; return false; }
fun anyAddress(addr: address): bool { addr; return true; }

/// Test predicate for `exitCode` — non-zero exit code
get fun `test predicate exitCode`() {
    val (counter, deployer) = setupTest();
    val msg = createMessage({
        bounce: false,
        value: ton("0.1"),
        dest: counter.address,
        body: beginCell().storeUint(0xDEADBEEF, 32).endCell(),
    });
    val res = net.send(deployer.address, msg);
    expect(res).toHaveTx({
        to: counter.address,
        exitCode: isNotZero,
    });
}

/// Test predicate for `success`
get fun `test predicate success`() {
    val (counter, deployer) = setupTest();
    val msg = createMessage({
        bounce: false,
        value: ton("0.1"),
        dest: counter.address,
        body: beginCell().storeUint(0xDEADBEEF, 32).endCell(),
    });
    val res = net.send(deployer.address, msg);
    expect(res).toHaveTx({
        to: counter.address,
        success: isFalse,
    });
}

/// Test mixed: plain value + predicate
get fun `test predicate mixed`() {
    val (counter, deployer) = setupTest();
    val res = counter.sendIncrease(deployer.address, 42);
    expect(res).toHaveSuccessfulTx({
        to: counter.address,
        from: anyAddress,
        exitCode: isZero,
        success: isTrue,
    });
}

/// Test negative: predicate that never matches
get fun `test predicate toNotHaveTx`() {
    val (counter, deployer) = setupTest();
    val res = counter.sendIncrease(deployer.address, 42);
    expect(res).toNotHaveTx({
        to: alwaysFalseAddr,
    });
}
