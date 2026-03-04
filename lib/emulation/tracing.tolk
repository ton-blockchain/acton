import "network"
import "../ffi/ffi"

/// Utilities for naming and organizing traces produced in tests.
struct tracing

/// Save a custom name for a trace chain produced by [net.send].
///
/// The custom name is used in the Test UI and gas/fee profiling reports.
///
/// Example:
/// ```tolk
/// val txs = net.send(deployer.address, msg);
/// tracing.save(txs, "deploy-counter");
/// expect(txs).toHaveAllSuccessfulTxs();
/// ```
fun tracing.save(txs: SendResultList, name: string): void {
    ffi.saveTraceName(txs, name);
}
