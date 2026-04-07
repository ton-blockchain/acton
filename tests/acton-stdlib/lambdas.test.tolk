import "@local-acton/emulation/network"
import "@local-acton/build/build"
import "@local-acton/ffi/ffi"
import "@local-acton/ffi/reverse-ffi"
import "@local-acton/testing/expect"
import "@local-acton/testing/transaction_expect"
import "@stdlib/reflection"
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

/// Test predicate for `exitCode` — inline lambda
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
        exitCode: fun(code: int32): bool { return code != 0; },
    });
}

/// Test predicate for `success` — inline lambda
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
        success: fun(s: bool): bool { return !s; },
    });
}

fun bind1_untyped(c: continuation, value: unknown): continuation asm "SWAP 1 -1 SETCONTARGS";

fun bindX(function: continuation, args: tuple): continuation {
    var c = function;
    var i = 0;
    while (i < args.size()) {
        c = bind1_untyped(c, args.get(i));
        i += 1;
    }
    return c;
}

get fun `test_capture_one`() {
    var a = 10;
    var cb = fun(x: int) { return x + a };
    return cb(5);
}


/// Test mixed: plain value + inline lambda predicates
get fun `test predicate mixed`() {
    val (counter, deployer) = setupTest();
    val res = counter.sendIncrease(deployer.address, 42);

    expect(res).toHaveSuccessfulTx({
        to: fun(addr: address): bool { return addr == counter.address; }
    });
}

fun array<T>.map<U>(self, f: (T -> U)): array<U> {
    var res: array<U> = [];
    var i = 0;
    repeat (self.size()) {
        res.push(f(self.get(i)));
        i += 1;
    }
    return res;
}

fun array<T>.filter(self, f: (T -> bool)): array<T> {
    var res: array<T> = [];
    var i = 0;
    repeat (self.size()) {
        val item = self.get(i);
        if (f(item)) {
            res.push(item);
        }
        i += 1;
    }
    return res;
}

get fun `test map and filter`() {
    val nums: array<int> = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10];

    // filter even, then square them
    val squared = nums
        .filter(fun(x: int): bool { return x % 2 == 0; })
        .map<int>(fun(x: int): int { return x * x; });
    // [2,4,6,8,10] -> [4,16,36,64,100]
    expect(squared.size()).toEqual(5);
    expect(squared.get(0)).toEqual(4);
    expect(squared.get(1)).toEqual(16);
    expect(squared.get(2)).toEqual(36);
    expect(squared.get(3)).toEqual(64);
    expect(squared.get(4)).toEqual(100);

    // chain: map then filter — triple all, keep only > 20
    val big = nums
        .map<int>(fun(x: int): int { return x * 3; })
        .filter(fun(x: int): bool { return x > 20; });
    // [3,6,9,12,15,18,21,24,27,30] -> [21,24,27,30]
    expect(big.size()).toEqual(4);
    expect(big.get(0)).toEqual(21);
    expect(big.get(3)).toEqual(30);

    // closures capturing outer state
    val threshold = 5;
    val multiplier = 7;
    val scaled = nums
        .filter(fun(x: int): bool { return x > threshold; })
        .map<int>(fun(x: int): int { return x * multiplier; });
    // [6,7,8,9,10] * 7 = [42,49,56,63,70]
    expect(scaled.size()).toEqual(5);
    expect(scaled.get(0)).toEqual(42);
    expect(scaled.get(4)).toEqual(70);
}

/// Test negative: inline lambda that never matches
get fun `test predicate toNotHaveTx`() {
    val (counter, deployer) = setupTest();
    val res = counter.sendIncrease(deployer.address, 42);
    expect(res).toNotHaveTx({
        to: fun(addr: address): bool { return false; },
    });
}
