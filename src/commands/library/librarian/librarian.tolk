tolk 1.3.0

import "@stdlib/gas-payments"

const DEFAULT_DURATION = 3600 * 24 * 365 * 1 // 1 year, can top-up in any time

fun contract.setLibraryCode(code: cell, mode: int): void
    asm "SETLIBCODE"

fun onInternalMessage(in: InMessage) {
    val libraryCode = contract.getData();

    val gasConsumedBeforeCalculation = getGasConsumedAtTheMoment();
    val (libraryRefs, libraryBits, _) = libraryCode.calculateSizeStrict(2048);
    val gasConsumedForCalculation = getGasConsumedAtTheMoment() - gasConsumedBeforeCalculation;

    val toReserve = calculateGasFeeWithoutFlatPrice(MASTERCHAIN, gasConsumedForCalculation) +
    calculateStorageFee(MASTERCHAIN, DEFAULT_DURATION, libraryBits, libraryRefs);

    reserveToncoinsOnBalance(toReserve, RESERVE_MODE_BOUNCE_ON_ACTION_FAIL);

    val excessesMessage = createMessage({
        dest: in.senderAddress,
        value: 0,
        bounce: false,
    });

    excessesMessage.send(SEND_MODE_CARRY_ALL_BALANCE);

    // if x = 2, the library is added as a public library (and becomes available to all smart
    // contracts if the current smart contract resides in the masterchain);
    contract.setLibraryCode(libraryCode, 2);

    // brick contract
    contract.setCodePostponed(createEmptyCell());
    contract.setData(createEmptyCell());
}
