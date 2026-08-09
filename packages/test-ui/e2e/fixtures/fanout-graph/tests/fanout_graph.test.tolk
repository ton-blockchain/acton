import "@acton/build"
import "@acton/io"
import "@acton/emulation/network"
import "@acton/emulation/testing"
import "@acton/testing/expect"
import "@contracts/messages"

struct Fanout {
    address: address
    init: ContractState
}

fun Fanout.fromEmptyData(): Fanout {
    val init = ContractState {
        code: build("fanout"),
        data: createEmptyCell(),
    };
    val address = AutoDeployAddress { stateInit: init }.calculateAddress();
    return Fanout { address, init };
}

fun deployFanout(sender: Treasury): Fanout {
    val fanout = Fanout.fromEmptyData();
    val deployMessage = createMessage({
        bounce: false,
        value: ton("1"),
        dest: { stateInit: fanout.init },
    });

    val deployResult = net.send(sender.address, deployMessage);
    expect(deployResult).toHaveSuccessfulDeploy({ to: fanout.address });

    return fanout;
}

fun Fanout.sendFanout(
    self,
    from: address,
    first: address,
    second: address,
    third: address,
): SendResultList {
    val triggerMessage = createMessage({
        bounce: false,
        value: ton("1"),
        dest: self.address,
        body: FanoutRequest {
            queryId: 1,
            first,
            second,
            third,
        },
    });
    return net.send(from, triggerMessage);
}

fun Fanout.sendSingle(self, from: address): SendResultList {
    val triggerMessage = createMessage({
        bounce: false,
        value: ton("1"),
        dest: self.address,
        body: SingleRequest { queryId: 10 },
    });
    return net.send(from, triggerMessage);
}

fun Fanout.sendChain(self, from: address, hops: int): SendResultList {
    val triggerMessage = createMessage({
        bounce: false,
        value: ton("2"),
        dest: self.address,
        body: ChainRequest {
            queryId: 2,
            hops,
        },
    });
    return net.send(from, triggerMessage);
}

fun Fanout.sendWideFanout(
    self,
    from: address,
    first: address,
    second: address,
    third: address,
): SendResultList {
    val triggerMessage = createMessage({
        bounce: false,
        value: ton("2"),
        dest: self.address,
        body: WideFanoutRequest {
            queryId: 5,
            first,
            second,
            third,
        },
    });
    return net.send(from, triggerMessage);
}

fun Fanout.sendBinary(self, from: address, left: address, right: address): SendResultList {
    val triggerMessage = createMessage({
        bounce: false,
        value: ton("2"),
        dest: self.address,
        body: BinaryRequest {
            queryId: 3,
            left,
            right,
        },
    });
    return net.send(from, triggerMessage);
}

fun Fanout.sendAsymmetric(
    self,
    from: address,
    first: address,
    second: address,
    final: address,
    chainHops: int,
): SendResultList {
    val triggerMessage = createMessage({
        bounce: false,
        value: ton("2"),
        dest: self.address,
        body: AsymmetricRequest {
            queryId: 6,
            chainHops,
            first,
            second,
            final,
        },
    });
    return net.send(from, triggerMessage);
}

fun Fanout.sendBranching(
    self,
    from: address,
    left: address,
    right: address,
    depth: int,
): SendResultList {
    val triggerMessage = createMessage({
        bounce: false,
        value: ton("6"),
        dest: self.address,
        body: BranchingRequest {
            queryId: 7,
            depth,
            left,
            right,
        },
    });
    return net.send(from, triggerMessage);
}

fun Fanout.sendFail(self, from: address): SendResultList {
    val triggerMessage = createMessage({
        bounce: false,
        value: ton("1"),
        dest: self.address,
        body: FailRequest { queryId: 8 },
    });
    return net.send(from, triggerMessage);
}

fun Fanout.sendMixedFailure(self, from: address, okTarget: address): SendResultList {
    val triggerMessage = createMessage({
        bounce: false,
        value: ton("2"),
        dest: self.address,
        body: MixedFailureRequest {
            queryId: 9,
            okTarget,
        },
    });
    return net.send(from, triggerMessage);
}

fun Fanout.sendExternalLog(self, from: address, target: address): SendResultList {
    val triggerMessage = createMessage({
        bounce: false,
        value: ton("1"),
        dest: self.address,
        body: ExternalLogRequest {
            queryId: 4,
            target,
        },
    });
    return net.send(from, triggerMessage);
}

get fun `test single transaction graph has no outgoing messages`() {
    val sender = testing.treasury("single-sender");
    val fanout = deployFanout(sender);

    val result = fanout.sendSingle(sender.address);
    result.giveName("single node");

    expect(result).toHaveSuccessfulTx({ from: sender.address, to: fanout.address });
    expect(result).toHaveAllSuccessfulTxs();
    println(result);
}

get fun `test fanout graph has three outgoing messages`() {
    val sender = testing.treasury("sender");
    val first = testing.treasury("first-recipient");
    val second = testing.treasury("second-recipient");
    val third = testing.treasury("third-recipient");
    val fanout = deployFanout(sender);

    val result = fanout.sendFanout(sender.address, first.address, second.address, third.address);
    result.giveName("fanout 3 leaves");

    expect(result).toHaveSuccessfulTx({ from: sender.address, to: fanout.address });
    expect(result).toHaveSuccessfulTx({ from: fanout.address, to: first.address });
    expect(result).toHaveSuccessfulTx({ from: fanout.address, to: second.address });
    expect(result).toHaveSuccessfulTx({ from: fanout.address, to: third.address });
    println(result);
}

get fun `test short chain graph has one self call`() {
    val sender = testing.treasury("short-chain-sender");
    val fanout = deployFanout(sender);

    val result = fanout.sendChain(sender.address, 1);
    result.giveName("short chain 1");

    expect(result).toHaveSuccessfulTx({ from: sender.address, to: fanout.address });
    expect(result).toHaveSuccessfulTx({ from: fanout.address, to: fanout.address });
    expect(result).toHaveAllSuccessfulTxs();
    println(result);
}

get fun `test chain graph has four sequential self calls`() {
    val sender = testing.treasury("chain-sender");
    val fanout = deployFanout(sender);

    val result = fanout.sendChain(sender.address, 4);
    result.giveName("chain 4");

    expect(result).toHaveSuccessfulTx({ from: sender.address, to: fanout.address });
    expect(result).toHaveSuccessfulTx({ from: fanout.address, to: fanout.address });
    expect(result).toHaveAllSuccessfulTxs();
    println(result);
}

get fun `test tall chain graph has eight sequential self calls`() {
    val sender = testing.treasury("tall-chain-sender");
    val fanout = deployFanout(sender);

    val result = fanout.sendChain(sender.address, 8);
    result.giveName("tall chain 8");

    expect(result).toHaveSuccessfulTx({ from: sender.address, to: fanout.address });
    expect(result).toHaveSuccessfulTx({ from: fanout.address, to: fanout.address });
    expect(result).toHaveAllSuccessfulTxs();
    println(result);
}

get fun `test wide fanout graph has six outgoing messages`() {
    val sender = testing.treasury("wide-sender");
    val first = testing.treasury("wide-first");
    val second = testing.treasury("wide-second");
    val third = testing.treasury("wide-third");
    val fanout = deployFanout(sender);

    val result = fanout.sendWideFanout(
        sender.address,
        first.address,
        second.address,
        third.address,
    );
    result.giveName("wide fanout 6");

    expect(result).toHaveSuccessfulTx({ from: sender.address, to: fanout.address });
    expect(result).toHaveSuccessfulTx({ from: fanout.address, to: first.address });
    expect(result).toHaveSuccessfulTx({ from: fanout.address, to: third.address });
    expect(result).toHaveAllSuccessfulTxs();
    println(result);
}

get fun `test binary graph branches and joins into leaves`() {
    val sender = testing.treasury("binary-sender");
    val left = testing.treasury("left-leaf");
    val right = testing.treasury("right-leaf");
    val fanout = deployFanout(sender);

    val result = fanout.sendBinary(sender.address, left.address, right.address);
    result.giveName("binary depth 2");

    expect(result).toHaveSuccessfulTx({ from: sender.address, to: fanout.address });
    expect(result).toHaveSuccessfulTx({ from: fanout.address, to: left.address });
    expect(result).toHaveSuccessfulTx({ from: fanout.address, to: right.address });
    expect(result).toHaveAllSuccessfulTxs();
    println(result);
}

get fun `test asymmetric graph mixes shallow leaves and tall branch`() {
    val sender = testing.treasury("asymmetric-sender");
    val first = testing.treasury("shallow-first");
    val second = testing.treasury("shallow-second");
    val final = testing.treasury("deep-final");
    val fanout = deployFanout(sender);

    val result = fanout.sendAsymmetric(
        sender.address,
        first.address,
        second.address,
        final.address,
        5,
    );
    result.giveName("asymmetric shallow plus deep");

    expect(result).toHaveSuccessfulTx({ from: sender.address, to: fanout.address });
    expect(result).toHaveSuccessfulTx({ from: fanout.address, to: first.address });
    expect(result).toHaveSuccessfulTx({ from: fanout.address, to: second.address });
    expect(result).toHaveSuccessfulTx({ from: fanout.address, to: final.address });
    expect(result).toHaveAllSuccessfulTxs();
    println(result);
}

get fun `test dense branching graph has many internal nodes`() {
    val sender = testing.treasury("branching-sender");
    val left = testing.treasury("branching-left-leaf");
    val right = testing.treasury("branching-right-leaf");
    val fanout = deployFanout(sender);

    val result = fanout.sendBranching(sender.address, left.address, right.address, 2);
    result.giveName("dense branching depth 2");

    expect(result).toHaveSuccessfulTx({ from: sender.address, to: fanout.address });
    expect(result).toHaveSuccessfulTx({ from: fanout.address, to: fanout.address });
    expect(result).toHaveSuccessfulTx({ from: fanout.address, to: left.address });
    expect(result).toHaveSuccessfulTx({ from: fanout.address, to: right.address });
    expect(result).toHaveAllSuccessfulTxs();
    println(result);
}

get fun `test tall dense branching graph stresses internal levels`() {
    val sender = testing.treasury("tall-branching-sender");
    val left = testing.treasury("tall-branching-left-leaf");
    val right = testing.treasury("tall-branching-right-leaf");
    val fanout = deployFanout(sender);

    val result = fanout.sendBranching(sender.address, left.address, right.address, 3);
    result.giveName("dense branching depth 3");

    expect(result).toHaveSuccessfulTx({ from: sender.address, to: fanout.address });
    expect(result).toHaveSuccessfulTx({ from: fanout.address, to: fanout.address });
    expect(result).toHaveSuccessfulTx({ from: fanout.address, to: left.address });
    expect(result).toHaveSuccessfulTx({ from: fanout.address, to: right.address });
    expect(result).toHaveAllSuccessfulTxs();
    println(result);
}

get fun `test graph has internal and external out children`() {
    val sender = testing.treasury("external-sender");
    val target = testing.treasury("external-target");
    val fanout = deployFanout(sender);

    val result = fanout.sendExternalLog(sender.address, target.address);
    result.giveName("internal plus external-out");

    expect(result).toHaveSuccessfulTx({ from: sender.address, to: fanout.address });
    expect(result).toHaveSuccessfulTx({ from: fanout.address, to: target.address });
    expect(result).toHaveAllSuccessfulTxs();
    println(result);
}

get fun `test root transaction fails without children`() {
    val sender = testing.treasury("failed-root-sender");
    val fanout = deployFanout(sender);

    val result = fanout.sendFail(sender.address);
    result.giveName("failed root");

    expect(result).toHaveFailedTx({
        from: sender.address,
        to: fanout.address,
        exitCode: 0xBEEF,
    });
    println(result);
}

get fun `test child transaction fails next to successful branch`() {
    val sender = testing.treasury("mixed-failure-sender");
    val okTarget = testing.treasury("mixed-failure-ok-target");
    val fanout = deployFanout(sender);

    val result = fanout.sendMixedFailure(sender.address, okTarget.address);
    result.giveName("successful and failed children");

    expect(result).toHaveSuccessfulTx({ from: sender.address, to: fanout.address });
    expect(result).toHaveSuccessfulTx({ from: fanout.address, to: okTarget.address });
    expect(result).toHaveFailedTx({
        from: fanout.address,
        to: fanout.address,
        exitCode: 0xBEEF,
    });
    println(result);
}
