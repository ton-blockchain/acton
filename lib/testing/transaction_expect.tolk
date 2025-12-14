/// Module for asserting properties of transactions.
///
/// This module extends the `Expectation` class specifically for `SendResultList` and `SendResult`.
/// It allows verifying the outcome of network operations, such as checking if a transaction succeeded,
/// if a contract was deployed, or if specific messages were emitted.
///
/// Examples:
/// ```tolk
/// val results = net.send(wallet.address, msg, SEND_MODE_PAY_FEES_SEPARATELY);
///
/// // Verify general success
/// expect(results).toHaveSuccessfulTx({
///     from: wallet.address,
///     to: contract.address
/// });
///
/// // Verify deployment
/// expect(results).toHaveSuccessfulDeploy({
///     to: contract.address
/// });
///
/// // Verify failure case
/// val failResults = net.send(wallet.address, invalidMsg, SEND_MODE_PAY_FEES_SEPARATELY);
/// expect(failResults).toHaveFailedTx({
///     to: contract.address,
///     exit_code: 102 // specific error code
/// });
/// ```

import "expect"
import "assert"
import "../fmt"
import "../emulation/network"
import "../ffi/ffi"
import "../types/transaction"

/// Expect the transaction list to have a successful deploy transaction.
/// 
/// Transaction is considered as "deploy" if previous status was "NonExisting" and end status was "Active".
/// 
/// Example:
/// 
/// ```tolk
/// expect(txs).toHaveSuccessfulDeploy({
///     to: counter.address,
/// });
/// ```
fun Expectation<SendResultList>.toHaveSuccessfulDeploy<Msg = never>(self, params: SearchParams = {}) {
    params.opcode = Msg.getDeclaredPackPrefix();
    params.exit_code = 0;
    params.deploy = true;

    val tx = ffi.findTransactionByParams(self.value, params.toTuple());
    if (tx == null) {
        ffi.failToFindTransactionByParams(
            self.location,
            "expect(<actual>).toHaveSuccessfulDeploy(<expected>)",
            self.value,
            params.toTuple()
        );
        throw 567;
    }
}

/// Expect the transaction list to have a successful transaction.
/// 
/// Transaction is considered as "successful" if exit code is 0, but exit code 
/// can be overridden by the `exit_code` parameter.
/// 
/// Example:
/// ```tolk
/// expect(txs).toHaveSuccessfulTx({
///     from: deployer.address,
///     to: counter.address,
/// });
/// ```
fun Expectation<SendResultList>.toHaveSuccessfulTx<Msg = never>(self, params: SearchParams = {}) {
    params.opcode = Msg.getDeclaredPackPrefix();
    if (params.exit_code == null) {
        params.exit_code = 0;
    }

    val tx = ffi.findTransactionByParams(self.value, params.toTuple());
    if (tx == null) {
        ffi.failToFindTransactionByParams(
            self.location,
            "expect(<actual>).toHaveSuccessfulTx(<expected>)",
            self.value,
            params.toTuple()
        );
        throw 567;
    }
}

/// Expect the transaction list to have a transaction with specified parameters.
///
/// Example:
/// ```tolk
/// expect(txs).toHaveTx({
///     from: deployer.address,
///     to: counter.address,
/// });
/// ```
fun Expectation<SendResultList>.toHaveTx<Msg = never>(self, params: SearchParams = {}) {
    params.opcode = Msg.getDeclaredPackPrefix();

    val tx = ffi.findTransactionByParams(self.value, params.toTuple());
    if (tx == null) {
        ffi.failToFindTransactionByParams(
            self.location,
            "expect(<actual>).toHaveTx(<expected>)",
            self.value,
            params.toTuple()
        );
        throw 567;
    }
}

/// Expect the transaction list to NOT have a transaction with specified parameters.
/// 
/// Example:
/// ```tolk
/// expect(txs).toNotHaveTx({
///     from: deployer.address,
///     to: counter.address,
/// });
/// ```
fun Expectation<SendResultList>.toNotHaveTx<Msg = never>(self, params: SearchParams = {}) {
    params.opcode = Msg.getDeclaredPackPrefix();

    val tx = ffi.findTransactionByParams(self.value, params.toTuple());
    if (tx != null) {
        ffi.failToNotFindTransactionByParams(
            self.location,
            "expect(<actual>).toNotHaveTx(<expected>)",
            self.value,
            params.toTuple()
        );
        throw 567;
    }
}

/// Expect the transaction list to have a **bounced** transaction.
/// 
/// Example:
/// ```tolk
/// expect(txs).toHaveBouncedTx({
///     from: deployer.address,
///     to: counter.address,
/// });
/// ```
fun Expectation<SendResultList>.toHaveBouncedTx<Msg = never>(self, params: SearchParams = {}) {
    params.opcode = Msg.getDeclaredPackPrefix();
    params.bounced = true;

    val tx = ffi.findTransactionByParams(self.value, params.toTuple());
    if (tx == null) {
        ffi.failToFindTransactionByParams(
            self.location,
            "expect(<actual>).toHaveBouncedTx(<expected>)",
            self.value,
            params.toTuple()
        );
        throw 567;
    }
}

/// Expect the transaction list to have a **failed** transaction. 
/// Exit code in parameters must be set to non zero value.
/// 
/// Example:
/// ```tolk
/// expect(txs).toHaveFailedTx({
///     from: deployer.address,
///     to: counter.address,
///     exit_code: 123,
/// });
/// ```
fun Expectation<SendResultList>.toHaveFailedTx<Msg = never>(self, params: SearchParams = {}) {
    params.opcode = Msg.getDeclaredPackPrefix();

    if (params.exit_code == null) {
        Assert.fail("expect(<actual>).toHaveFailedTx(params) must have not null exit_code parameter in params");
    }
    if (params.exit_code == 0) {
        Assert.fail("expect(<actual>).toHaveFailedTx(params) must have non zero exit_code parameter in params");
    }

    val tx = ffi.findTransactionByParams(self.value, params.toTuple());
    if (tx == null) {
        ffi.failToFindTransactionByParams(
            self.location,
            "expect(<actual>).toHaveFailedTx(<expected>)",
            self.value,
            params.toTuple()
        );
        throw 567;
    }
}

/// Expect the transaction to emit an external message of given type.
/// 
/// Example:
/// ```tolk
/// expect(txs).toEmitExternalMessage<IncreaseCounter>();
/// ```
fun Expectation<SendResultList>.toEmitExternalMessage<Msg>(self) {
    var i = 0;
    while (i < self.value.size()) {
        val result = self.value.at(i);

        var j = 0;
        while (j < result.externals.size()) {
            val external = result.externals.atOrNull<Msg>(j);
            if (external != null) {
                // message found
                return
            }
            j += 1;
        }

        i += 1;
    }

    Assert.fail(
        format3(
            "expect(<actual>).toEmitExternalMessage<{}>(): Cannot find external message {} (opcode: 0x{:x})",
            Msg.typeName(),
            Msg.typeName(),
            Msg.getDeclaredPackPrefix()
        ),
        self.location
    );
}

/// Expect the transaction to consume less than the expected amount of gas.
/// 
/// Example:
/// ```tolk
/// val result = net.send(deployer.address, message, SEND_MODE_PAY_FEES_SEPARATELY);
/// expect(result.at(0)).toConsumeLessThan(1000);
/// ```
fun Expectation<SendResult>.toConsumeLessThan(self, amount: int) {
    ffi.assert_comparison(
        self.value.gas_used,
        amount,
        "<",
        "expect(<actual>).toConsumeLessThan(<expected>)",
        self.location
    );
}

fun never.getDeclaredPackPrefix() {
    return null;
}

fun never.getDeclaredPackPrefixLen() {
    return null;
}
