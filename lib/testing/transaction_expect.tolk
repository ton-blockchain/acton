import "expect"
import "assert"
import "../emulation/network"
import "../ffi/ffi"
import "../types/transaction"

/// Expect the transaction list to have a successful deploy transaction.
/// 
/// Transaction is considered as "deploy" if previous status was "NonExisting" and end status was "Active".
/// 
/// Example:
/// 
/// ```
/// expect(txs).toHaveSuccessfulDeploy({
///     to: counter.address,
/// });
/// ```
fun Expectation<SendResultList>.toHaveSuccessfulDeploy(self, params: SearchParams) {
    params.exit_code = 0;
    params.deploy = true;

    val tx = ffi.findTransactionByParams(self.value, params.__toTuple());
    if (tx == null) {
        ffi.failToFindTransactionByParams(
            self.location,
            "expect(<actual>).toHaveSuccessfulDeploy(<expected>)",
            self.value,
            params.__toTuple()
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
/// ```
/// expect(txs).toHaveSuccessfulTx({
///     from: deployer.address,
///     to: counter.address,
/// });
/// ```
fun Expectation<SendResultList>.toHaveSuccessfulTx(self, params: SearchParams) {
    if (params.exit_code == null) {
        params.exit_code = 0;
    }

    val tx = ffi.findTransactionByParams(self.value, params.__toTuple());
    if (tx == null) {
        ffi.failToFindTransactionByParams(
            self.location,
            "expect(<actual>).toHaveSuccessfulTx(<expected>)",
            self.value,
            params.__toTuple()
        );
        throw 567;
    }
}

/// Expect the transaction list to NOT have a transaction with specified parameters.
/// 
/// Example:
/// ```
/// expect(txs).toNotHaveTx({
///     from: deployer.address,
///     to: counter.address,
/// });
/// ```
fun Expectation<SendResultList>.toNotHaveTx(self, params: SearchParams) {
    val tx = ffi.findTransactionByParams(self.value, params.__toTuple());
    if (tx != null) {
        ffi.failToNotFindTransactionByParams(
            self.location,
            "expect(<actual>).toNotHaveTx(<expected>)",
            self.value,
            params.__toTuple()
        );
        throw 567;
    }
}

/// Expect the transaction list to have a **bounced** transaction.
/// 
/// Example:
/// ```
/// expect(txs).toHaveBouncedTx({
///     from: deployer.address,
///     to: counter.address,
/// });
/// ```
fun Expectation<SendResultList>.toHaveBouncedTx(self, params: SearchParams) {
    params.bounced = true;

    val tx = ffi.findTransactionByParams(self.value, params.__toTuple());
    if (tx == null) {
        ffi.failToFindTransactionByParams(
            self.location,
            "expect(<actual>).toHaveBouncedTx(<expected>)",
            self.value,
            params.__toTuple()
        );
        throw 567;
    }
}

/// Expect the transaction list to have a **failed** transaction. 
/// Exit code in parameters must be set to non zero value.
/// 
/// Example:
/// ```
/// expect(txs).toHaveFailedTx({
///     from: deployer.address,
///     to: counter.address,
///     exit_code: 123,
/// });
/// ```
fun Expectation<SendResultList>.toHaveFailedTx(self, params: SearchParams) {
    if (params.exit_code == null) {
        Assert.fail("expect(<actual>).toHaveFailedTx(params) must have not null exit_code parameter in params");
    }
    if (params.exit_code == 0) {
        Assert.fail("expect(<actual>).toHaveFailedTx(params) must have non zero exit_code parameter in params");
    }

    val tx = ffi.findTransactionByParams(self.value, params.__toTuple());
    if (tx == null) {
        ffi.failToFindTransactionByParams(
            self.location,
            "expect(<actual>).toHaveFailedTx(<expected>)",
            self.value,
            params.__toTuple()
        );
        throw 567;
    }
}

/// Expect the transaction to consume less than the expected amount of gas.
/// 
/// Example:
/// ```
/// let result = net.send(deployer.address, message, SEND_MODE_PAY_FEES_SEPARATELY);
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
