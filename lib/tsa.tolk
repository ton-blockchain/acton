/// Module for TSA custom checker intrinsics.
///
/// This namespace is intended for checker code executed by TSA, not for
/// on-chain contracts. It provides a typed wrapper over TSA-specific `EXTCALL`
/// opcodes, including the generic tuple-based `callRaw` / `call` surface.
///
/// Available since trunk.
///
/// Example:
/// ```tolk
/// import "@acton/tsa"
///
/// fun onInternalMessage() {
///     val x = tsa.mkInt(257, true);
///     val y = tsa.mkInt(257, true);
///     val sorted: (int, int) = tsa.call(1, 10, (x, y));
///     val (a, b) = sorted;
///     assert(a <= b) throw 100;
/// }
/// ```

/// Namespace for TSA checker intrinsics.
struct tsa

/// Handle for a specific analyzed contract inside a TSA checker.
struct TsaContract<TStorage = cell> {
    id: int
}

/// Bind helper methods to a specific TSA contract id.
///
/// If `initialData` is provided, the contract storage is initialized via
/// [tsa.setData] before the handle is returned.
fun tsa.contract<TStorage = cell>(
    contractId: int,
    initialData: TStorage? = null,
): TsaContract<TStorage> {
    if (initialData != null) {
        tsa.setData(contractId, initialData.toCell());
    }

    return TsaContract<TStorage> { id: contractId };
}

/// Start an assumption scope that discards executions with failures.
fun tsa.forbidFailures(): void
    asm "701 EXTCALL";

/// End the current assumption scope.
fun tsa.allowFailures(): void
    asm "702 EXTCALL";

/// Add an assumption to the current symbolic path.
fun tsa.assume(condition: bool): void
    asm "703 EXTCALL";

/// Compatibility alias with a more checker-oriented name.
fun tsa.assertThat(condition: bool): void {
    tsa.assume(condition);
}

/// Add a negated assumption to the current symbolic path.
fun tsa.assumeNot(condition: bool): void
    asm "704 EXTCALL";

/// Compatibility alias with a more checker-oriented name.
fun tsa.assertThatNot(condition: bool): void {
    tsa.assumeNot(condition);
}

/// Create a symbolic integer.
fun tsa.mkInt(bits: int, signed: bool): int
    asm "100 EXTCALL";

/// Capture a symbolic or computed value for SARIF output.
fun tsa.saveValue<T>(value: T, valueId: int): void
    asm "705 EXTCALL";

/// Compatibility alias with a more explicit reporting-oriented name.
fun tsa.saveValueInReport<T>(value: T, valueId: int): void {
    tsa.saveValue(value, valueId);
}

/// Send an internal message without an explicit body to another contract under test.
fun tsa.sendInternalMessageEmpty(contractId: int, inputId: int): void
    asm "706 EXTCALL";

/// Send a typed internal message body to another contract under test.
///
/// This helper serializes `body` with `toCell()` and passes its full slice as
/// the message body. Use [tsa.sendInternalMessageWithBody] when you already
/// have a prebuilt `slice`.
fun tsa.sendInternalMessage<TBody>(body: TBody, contractId: int, inputId: int): void {
    tsa.sendInternalMessageWithBody(body.toCell().beginParse(), contractId, inputId);
}

/// Read the current persistent data cell of an analyzed contract.
fun tsa.getData(contractId: int): cell
    asm "707 EXTCALL";

/// Send an external message without an explicit body to another contract under test.
fun tsa.sendExternalMessageEmpty(contractId: int, inputId: int): void
    asm "708 EXTCALL";

/// Send a typed external message body to another contract under test.
///
/// This helper serializes `body` with `toCell()` and passes its full slice as
/// the message body. Use [tsa.sendExternalMessageWithBody] when you already
/// have a prebuilt `slice`.
fun tsa.sendExternalMessage<TBody>(body: TBody, contractId: int, inputId: int): void {
    tsa.sendExternalMessageWithBody(body.toCell().beginParse(), contractId, inputId);
}

/// Read the balance of an analyzed contract by TSA contract id.
fun tsa.getBalance(contractId: int): int
    asm "709 EXTCALL";

/// Replace the current persistent data cell of an analyzed contract.
fun tsa.setData(contractId: int, value: cell): void
    asm "710 EXTCALL";

/// Compatibility alias for the old TVM register-oriented name.
fun tsa.getC4(contractId: int): cell {
    return tsa.getData(contractId);
}

/// Compatibility alias for the old TVM register-oriented name.
fun tsa.setC4(contractId: int, value: cell): void {
    tsa.setData(contractId, value);
}

/// Replace a concrete address with a symbolic random address.
fun tsa.makeAddressRandom(address: slice): void
    asm "711 EXTCALL";

/// Mark a slice as independent from random addresses.
fun tsa.makeSliceIndependentFromRandomAddresses(independentSlice: slice): void
    asm "712 EXTCALL";

/// Check whether the input identified by `inputId` was accepted.
fun tsa.inputWasAccepted(inputId: int): bool
    asm "713 EXTCALL";

/// Send an internal message with an explicit body slice.
fun tsa.sendInternalMessageWithBody(body: slice, contractId: int, inputId: int): void
    asm "714 EXTCALL";

/// Send an external message with an explicit body slice.
fun tsa.sendExternalMessageWithBody(body: slice, contractId: int, inputId: int): void
    asm "715 EXTCALL";

/// Low-level generic TSA call.
///
/// `args` is a Tolk tuple transport container, so raw calls look like
/// `tsa.callRaw(contractId, methodId, [x, y])`.
/// The return value is also a tuple, for example `[a, b]`.
fun tsa.callRaw(contractId: int, methodId: int, args: tuple): tuple
    asm "716 EXTCALL";

/// High-level typed wrapper over [tsa.callRaw].
///
/// - tensors like `(x, y)` are converted to tuple transport with `toTuple()`
/// - raw tuples use `[]`, for example `tsa.callRaw(1, 42, [x, y])`
/// - structs also work via `toTuple()`
/// - the result type is usually inferred from the expected context
fun tsa.call<Args, Ret>(contractId: int, methodId: int, args: Args): Ret {
    val raw = tsa.callRaw(contractId, methodId, args.toTuple());
    return Ret.fromTuple(raw);
}

/// Send an internal message without an explicit body to this contract.
fun TsaContract<TStorage>.sendInternalMessageEmpty(self, inputId: int): void {
    tsa.sendInternalMessageEmpty(self.id, inputId);
}

/// Send a typed internal message body to this contract.
fun TsaContract<TStorage>.sendInternalMessage<TBody>(self, body: TBody, inputId: int): void {
    tsa.sendInternalMessage(body, self.id, inputId);
}

/// Send an internal message with an explicit body slice to this contract.
fun TsaContract<TStorage>.sendInternalMessageWithBody(self, body: slice, inputId: int): void {
    tsa.sendInternalMessageWithBody(body, self.id, inputId);
}

/// Send an external message without an explicit body to this contract.
fun TsaContract<TStorage>.sendExternalMessageEmpty(self, inputId: int): void {
    tsa.sendExternalMessageEmpty(self.id, inputId);
}

/// Send a typed external message body to this contract.
fun TsaContract<TStorage>.sendExternalMessage<TBody>(self, body: TBody, inputId: int): void {
    tsa.sendExternalMessage(body, self.id, inputId);
}

/// Send an external message with an explicit body slice to this contract.
fun TsaContract<TStorage>.sendExternalMessageWithBody(self, body: slice, inputId: int): void {
    tsa.sendExternalMessageWithBody(body, self.id, inputId);
}

/// Read the current persistent data cell of this contract.
fun TsaContract<TStorage>.getData(self): cell {
    return tsa.getData(self.id);
}

/// Replace the current persistent data cell of this contract.
fun TsaContract<TStorage>.setData(self, value: cell): void {
    tsa.setData(self.id, value);
}

/// Read storage from this contract using the storage type attached to the handle.
fun TsaContract<TStorage>.getStorage(self): TStorage {
    return TStorage.fromCell(self.getData());
}

/// Replace storage in this contract using the storage type attached to the handle.
fun TsaContract<TStorage>.setStorage(self, value: TStorage): void {
    self.setData(value.toCell());
}

/// Compatibility alias for the old TVM register-oriented name.
fun TsaContract<TStorage>.getC4(self): cell {
    return self.getData();
}

/// Compatibility alias for the old TVM register-oriented name.
fun TsaContract<TStorage>.setC4(self, value: cell): void {
    self.setData(value);
}

/// Read the current balance of this contract.
fun TsaContract<TStorage>.getBalance(self): int {
    return tsa.getBalance(self.id);
}

/// Low-level generic call against this contract.
fun TsaContract<TStorage>.callRaw(self, methodId: int, args: tuple): tuple {
    return tsa.callRaw(self.id, methodId, args);
}

/// High-level typed call against this contract.
fun TsaContract<TStorage>.call<Args, Ret>(self, methodId: int, args: Args): Ret {
    return tsa.call(self.id, methodId, args);
}
