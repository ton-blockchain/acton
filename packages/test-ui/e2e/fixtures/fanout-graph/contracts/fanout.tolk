import "messages"

contract Fanout {
    author: "Acton"
    version: "1.0.0"
    description: "Transaction graph fanout fixture"
    incomingMessages: GraphMessage
}

type GraphMessage =
    | FanoutRequest
    | ChainRequest
    | ChainStep
    | BinaryRequest
    | BinaryBranch
    | ExternalLogRequest
    | SingleRequest
    | WideFanoutRequest
    | AsymmetricRequest
    | AsymmetricStep
    | BranchingRequest
    | BranchingNode
    | FailRequest
    | MixedFailureRequest
    | FailStep

fun sendNotice(dest: address, queryId: int, branch: int) {
    val notice = createMessage({
        bounce: false,
        value: ton("0.05"),
        dest,
        body: FanoutNotice { queryId, branch },
    });
    notice.send(SEND_MODE_PAY_FEES_SEPARATELY);
}

fun sendChainStep(queryId: int, remaining: int) {
    val nextStep = createMessage({
        bounce: false,
        value: ton("0.15"),
        dest: contract.getAddress(),
        body: ChainStep { queryId, remaining },
    });
    nextStep.send(SEND_MODE_PAY_FEES_SEPARATELY);
}

fun sendBinaryBranch(queryId: int, branch: int, target: address) {
    val branchMessage = createMessage({
        bounce: false,
        value: ton("0.2"),
        dest: contract.getAddress(),
        body: BinaryBranch { queryId, branch, target },
    });
    branchMessage.send(SEND_MODE_PAY_FEES_SEPARATELY);
}

fun sendAsymmetricStep(queryId: int, remaining: int, final: address) {
    val nextStep = createMessage({
        bounce: false,
        value: ton("0.12"),
        dest: contract.getAddress(),
        body: AsymmetricStep { queryId, remaining, final },
    });
    nextStep.send(SEND_MODE_PAY_FEES_SEPARATELY);
}

fun sendBranchingNode(queryId: int, remaining: int, branch: int, left: address, right: address) {
    val nextNode = createMessage({
        bounce: false,
        value: ton("0.2"),
        dest: contract.getAddress(),
        body: BranchingNode { queryId, remaining, branch, left, right },
    });
    nextNode.send(SEND_MODE_PAY_FEES_SEPARATELY);
}

fun sendFailStep(queryId: int) {
    val failStep = createMessage({
        bounce: false,
        value: ton("0.15"),
        dest: contract.getAddress(),
        body: FailStep { queryId },
    });
    failStep.send(SEND_MODE_PAY_FEES_SEPARATELY);
}

fun emitExternalLog(queryId: int) {
    val logMessage = createExternalLogMessage({
        dest: createAddressNone(),
        body: ExternalLogNotice { queryId },
    });
    logMessage.send(SEND_MODE_REGULAR);
}

fun onInternalMessage(in: InMessage) {
    if (in.body.isEmpty()) {
        return;
    }

    val msg = lazy GraphMessage.fromSlice(in.body);
    match (msg) {
        SingleRequest => {}

        FanoutRequest => {
            sendNotice(msg.first, msg.queryId, 1);
            sendNotice(msg.second, msg.queryId, 2);
            sendNotice(msg.third, msg.queryId, 3);
        }

        WideFanoutRequest => {
            sendNotice(msg.first, msg.queryId, 1);
            sendNotice(msg.second, msg.queryId, 2);
            sendNotice(msg.third, msg.queryId, 3);
            sendNotice(msg.first, msg.queryId, 4);
            sendNotice(msg.second, msg.queryId, 5);
            sendNotice(msg.third, msg.queryId, 6);
        }

        ChainRequest => {
            if (msg.hops > 0) {
                sendChainStep(msg.queryId, msg.hops - 1);
            }
        }

        ChainStep => {
            if (msg.remaining > 0) {
                sendChainStep(msg.queryId, msg.remaining - 1);
            }
        }

        BinaryRequest => {
            sendBinaryBranch(msg.queryId, 1, msg.left);
            sendBinaryBranch(msg.queryId, 2, msg.right);
        }

        BinaryBranch => {
            sendNotice(msg.target, msg.queryId, msg.branch);
        }

        AsymmetricRequest => {
            sendNotice(msg.first, msg.queryId, 1);
            sendNotice(msg.second, msg.queryId, 2);
            sendAsymmetricStep(msg.queryId, msg.chainHops, msg.final);
        }

        AsymmetricStep => {
            if (msg.remaining > 0) {
                sendAsymmetricStep(msg.queryId, msg.remaining - 1, msg.final);
            } else {
                sendNotice(msg.final, msg.queryId, 3);
            }
        }

        BranchingRequest => {
            sendBranchingNode(msg.queryId, msg.depth, 1, msg.left, msg.right);
            sendBranchingNode(msg.queryId, msg.depth, 2, msg.left, msg.right);
        }

        BranchingNode => {
            if (msg.remaining > 0) {
                sendBranchingNode(
                    msg.queryId,
                    msg.remaining - 1,
                    msg.branch * 2,
                    msg.left,
                    msg.right,
                );
                sendBranchingNode(
                    msg.queryId,
                    msg.remaining - 1,
                    msg.branch * 2 + 1,
                    msg.left,
                    msg.right,
                );
            } else {
                sendNotice(msg.left, msg.queryId, msg.branch);
                sendNotice(msg.right, msg.queryId, msg.branch + 100);
            }
        }

        FailRequest => {
            throw 0xBEEF;
        }

        MixedFailureRequest => {
            sendNotice(msg.okTarget, msg.queryId, 1);
            sendFailStep(msg.queryId);
        }

        FailStep => {
            throw 0xBEEF;
        }

        ExternalLogRequest => {
            sendNotice(msg.target, msg.queryId, 1);
            emitExternalLog(msg.queryId);
        }

        else => {
            throw 0xFFFF;
        }
    }
}

fun onBouncedMessage(_: InMessageBounced) {}
