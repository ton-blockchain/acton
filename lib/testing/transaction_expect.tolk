/// Module for asserting properties of transactions.
///
/// This module extends the `Expectation` class specifically for `SendResultList` and `SendResult`.
/// It allows verifying the outcome of network operations, such as checking if a transaction succeeded,
/// if a contract was deployed, or if specific messages were emitted.
///
/// Matchers accept `SearchParams`; all non-null fields are used as filters.
/// Generic matcher variants (`<Msg>`) additionally inject opcode filtering from `Msg` declared prefix (if present).
///
/// Examples:
/// ```tolk
/// val results = net.send(wallet.address, msg);
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
/// val failResults = net.send(wallet.address, invalidMsg);
/// expect(failResults).toHaveFailedTx({
///     to: contract.address,
///     exitCode: 102 // specific error code
/// });
/// ```

import "@stdlib/reflection"
import "../fmt"
import "../emulation/network"
import "../ffi/ffi"
import "assert"
import "expect"

/// Expect the transaction list to have a successful deploy transaction.
///
/// Transaction is considered as "deploy" if previous status was "NonExisting" and end status was "Active".
/// If generic `Msg` is provided and has declared prefix, matcher also filters by `params.opcode`.
///
/// Example:
///
/// ```tolk
/// expect(txs).toHaveSuccessfulDeploy({
///     to: counter.address,
/// });
/// ```
fun Expectation<SendResultList>.toHaveSuccessfulDeploy<Msg = unknown>(
    self,
    params: SearchParams = {},
): void {
    val declaredPrefix = Msg.getDeclaredPackPrefix();
    if (declaredPrefix != -1) {
        params.opcode = declaredPrefix;
    }
    params.success = true;
    params.deploy = true;

    val tx = ffi.findTransactionByParams(self.value, params.toTuple());
    if (tx == null) {
        ffi.failToFindTransactionByParams(
            self.location,
            "expect(<actual>).toHaveSuccessfulDeploy(<expected>)",
            self.value,
            params.toTuple(),
        );
        throw 567;
    }
}

/// Expect the transaction list to have a successful transaction.
///
/// Matcher sets `params.success = true`.
/// Runtime "success" means both compute phase and action phase are successful.
///
/// By default matcher also requires `params.exitCode = 0` when `exitCode` is not provided.
/// This default can be overridden with explicit `params.exitCode`.
///
/// If generic `Msg` is provided and has declared prefix, matcher also filters by `params.opcode`.
///
/// Example:
/// ```tolk
/// expect(txs).toHaveSuccessfulTx({
///     from: deployer.address,
///     to: counter.address,
/// });
/// ```
fun Expectation<SendResultList>.toHaveSuccessfulTx<Msg = unknown>(
    self,
    params: SearchParams = {},
): void {
    val declaredPrefix = Msg.getDeclaredPackPrefix();
    if (declaredPrefix != -1) {
        params.opcode = declaredPrefix;
    }
    params.success = true;
    if (params.exitCode == null) {
        params.exitCode = 0;
    }

    val tx = ffi.findTransactionByParams(self.value, params.toTuple());
    if (tx == null) {
        ffi.failToFindTransactionByParams(
            self.location,
            "expect(<actual>).toHaveSuccessfulTx(<expected>)",
            self.value,
            params.toTuple(),
        );
        throw 567;
    }
}

/// Expect the transaction list to have a transaction with specified parameters.
///
/// All non-null `SearchParams` fields are applied as filters.
///
/// If generic `Msg` is provided and has declared prefix, matcher also filters by `params.opcode`.
///
/// Example:
/// ```tolk
/// expect(txs).toHaveTx({
///     from: deployer.address,
///     to: counter.address,
///     actionExitCode: 37,
/// });
/// ```
fun Expectation<SendResultList>.toHaveTx<Msg = unknown>(self, params: SearchParams = {}): void {
    val declaredPrefix = Msg.getDeclaredPackPrefix();
    if (declaredPrefix != -1) {
        params.opcode = declaredPrefix;
    }

    val tx = ffi.findTransactionByParams(self.value, params.toTuple());
    if (tx == null) {
        ffi.failToFindTransactionByParams(
            self.location,
            "expect(<actual>).toHaveTx(<expected>)",
            self.value,
            params.toTuple(),
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
fun Expectation<SendResultList>.toNotHaveTx<Msg = unknown>(self, params: SearchParams = {}): void {
    val declaredPrefix = Msg.getDeclaredPackPrefix();
    if (declaredPrefix != -1) {
        params.opcode = declaredPrefix;
    }

    val tx = ffi.findTransactionByParams(self.value, params.toTuple());
    if (tx != null) {
        ffi.failToNotFindTransactionByParams(
            self.location,
            "expect(<actual>).toNotHaveTx(<expected>)",
            self.value,
            params.toTuple(),
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
fun Expectation<SendResultList>.toHaveBouncedTx<Msg = unknown>(
    self,
    params: SearchParams = {},
): void {
    val declaredPrefix = Msg.getDeclaredPackPrefix();
    if (declaredPrefix != -1) {
        params.opcode = declaredPrefix;
    }
    params.bounced = true;

    val tx = ffi.findTransactionByParams(self.value, params.toTuple());
    if (tx == null) {
        ffi.failToFindTransactionByParams(
            self.location,
            "expect(<actual>).toHaveBouncedTx(<expected>)",
            self.value,
            params.toTuple(),
        );
        throw 567;
    }
}

/// Expect the transaction list to have a **failed** transaction.
///
/// This matcher sets `params.success = false`.
/// `params.exitCode` is required and must be non-zero.
/// If `exitCode` is null or `0`, matcher fails immediately before transaction search.
///
/// Any additional non-null `SearchParams` fields further narrow the search.
/// If generic `Msg` is provided and has declared prefix, matcher also filters by `params.opcode`.
///
/// Example:
/// ```tolk
/// expect(txs).toHaveFailedTx({
///     from: deployer.address,
///     to: counter.address,
///     exitCode: 123,
/// });
/// ```
fun Expectation<SendResultList>.toHaveFailedTx<Msg = unknown>(
    self,
    params: SearchParams = {},
): void {
    val declaredPrefix = Msg.getDeclaredPackPrefix();
    if (declaredPrefix != -1) {
        params.opcode = declaredPrefix;
    }
    params.success = false;
    if (params.exitCode == null) {
        Assert.fail(
            "expect(<actual>).toHaveFailedTx(params) must have not null exitCode parameter in params",
        );
    }
    if (params.exitCode == 0) {
        Assert.fail(
            "expect(<actual>).toHaveFailedTx(params) must have non zero exitCode parameter in params",
        );
    }

    val tx = ffi.findTransactionByParams(self.value, params.toTuple());
    if (tx == null) {
        ffi.failToFindTransactionByParams(
            self.location,
            "expect(<actual>).toHaveFailedTx(<expected>)",
            self.value,
            params.toTuple(),
        );
        throw 567;
    }
}

/// Expect all transactions in the list to be successful.
///
/// Example:
/// ```tolk
/// val results = net.send(wallet.address, msg);
/// expect(results).toHaveAllSuccessfulTxs();
/// ```
fun Expectation<SendResultList>.toHaveAllSuccessfulTxs(self): void {
    val params = SearchParams { success: false };
    // trying to find some transaction with success=false
    val tx = ffi.findTransactionByParams(self.value, params.toTuple());
    if (tx != null) {
        // we found one, so matcher fail
        ffi.failToNotFindTransactionByParams(
            self.location,
            "expect(<actual>).toHaveAllSuccessfulTxs(<expected>)",
            self.value,
            params.toTuple(),
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
fun Expectation<SendResultList>.toEmitExternalMessage<Msg>(self): void {
    var i = 0;
    while (i < self.value.size()) {
        val result = self.value.at(i);

        var j = 0;
        while (j < result.externals.size()) {
            val external = result.externals.atOrNull<Msg>(j);
            if (external != null) {
                // message found
                return;
            }
            j += 1;
        }

        i += 1;
    }

    Assert.fail(
        format3(
            "expect(<actual>).toEmitExternalMessage<{}>(): Cannot find external message {} (opcode: 0x{:x})",
            reflect.typeNameOf<Msg>(),
            reflect.typeNameOf<Msg>(),
            reflect.serializationPrefixOf<Msg>(),
        ),
        self.location,
    );
}

/// Expect the transaction to consume less than the expected amount of gas.
///
/// Example:
/// ```tolk
/// val result = net.send(deployer.address, message);
/// expect(result.at(0)).toConsumeLessThan(1000);
/// ```
fun Expectation<SendResult>.toConsumeLessThan(self, amount: int): void {
    ffi.assertComparison(
        self.value.gasUsed,
        amount,
        "<",
        "expect(<actual>).toConsumeLessThan(<expected>)",
        self.location,
    );
}

fun T.getDeclaredPackPrefix(): int {
    return reflect.serializationPrefixOf<T>();
}

fun unknown.getDeclaredPackPrefix(): int {
    return -1;
}

fun unknown.getDeclaredPackPrefixLen(): int {
    return -1;
}
