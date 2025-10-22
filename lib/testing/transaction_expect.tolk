import "expect"
import "assert"
import "../types/transaction"

/// Represents a set of parameters to search for a transaction in the transaction list.
struct SearchParams {
    /// Destination address of the transaction.
    to: address
    /// Source address of the transaction.
    from: address? = null
    /// Exit code of the transaction.
    exit_code: int32? = null
    /// Whether the transaction is a deploy transaction.
    /// 
    /// Deploy transaction is a transaction that changes the `from` account status from "NonExisting" to "Active".
    deploy: bool? = null
    /// Whether the transaction is a bounced transaction.
    bounced: bool? = null
}

/// Expect the transaction list to have a successful deploy transaction.
/// 
/// Transaction is considered as "deploy" if previous status was "NonExisting" and end status was "Active".
/// 
/// Example:
/// 
/// ```
/// expect(txs).toHaveSuccesfulDeploy({
///     to: counter.address,
/// });
/// ```
fun Expectation<TransactionList>.toHaveSuccesfulDeploy(self, params: SearchParams) {
    params.exit_code = 0;
    params.deploy = true;

    val tx = __findTransactionByParams(self.value, params.__toTuple());
    if (tx == null) {
        __failToFindTransactionByParams(
            self.location,
            "expect(actual).toHaveSuccesfulDeploy(expected)",
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
/// expect(txs).toHaveSuccesfulTx({
///     from: deployer.address,
///     to: counter.address,
/// });
/// ```
fun Expectation<TransactionList>.toHaveSuccesfulTx(self, params: SearchParams) {
    if (params.exit_code == null) {
        params.exit_code = 0;
    }

    val tx = __findTransactionByParams(self.value, params.__toTuple());
    if (tx == null) {
        __failToFindTransactionByParams(
            self.location,
            "expect(actual).toHaveSuccesfulTx(expected)",
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
fun Expectation<TransactionList>.toNotHaveTx(self, params: SearchParams) {
    val tx = __findTransactionByParams(self.value, params.__toTuple());
    if (tx != null) {
        __failToNotFindTransactionByParams(
            self.location,
            "expect(actual).toNotHaveTx(expected)",
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
fun Expectation<TransactionList>.toHaveBouncedTx(self, params: SearchParams) {
    params.bounced = true;

    val tx = __findTransactionByParams(self.value, params.__toTuple());
    if (tx == null) {
        __failToFindTransactionByParams(
            self.location,
            "expect(actual).toHaveBouncedTx(expected)",
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
fun Expectation<TransactionList>.toHaveFailedTx(self, params: SearchParams) {
    if (params.exit_code == null) {
        Assert.fail("expect(actual).toHaveFailedTx(params) must have not null exit_code parameter in params");
    }
    if (params.exit_code == 0) {
        Assert.fail("expect(actual).toHaveFailedTx(params) must have non zero exit_code parameter in params");
    }

    val tx = __findTransactionByParams(self.value, params.__toTuple());
    if (tx == null) {
        __failToFindTransactionByParams(
            self.location,
            "expect(actual).toHaveFailedTx(expected)",
            self.value,
            params.__toTuple()
        );
        throw 567;
    }
}

// fun Expectation<TransactionList>.findMessage<Message>(self, criteria: SearchParams): Transaction {
//     // TODO
// }
//
// fun Expectation<TransactionList>.findExternalMessage<Message>(self, criteria: SearchParams): Transaction {
//     // TODO
// }

fun __findTransactionByParams(txs: tuple, params: tuple): tuple?
    asm "10 EXTCALL"

fun __failToFindTransactionByParams(location: slice, message: slice, txs: tuple, params: tuple): tuple?
    asm "103 EXTCALL"

fun __failToNotFindTransactionByParams(location: slice, message: slice, txs: tuple, params: tuple): tuple?
    asm "104 EXTCALL"
