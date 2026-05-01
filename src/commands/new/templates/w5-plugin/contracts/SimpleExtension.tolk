import "@stdlib/gas-payments"
import "@contracts/errors"
import "@contracts/types"
import "@contracts/w5-types"
import "types"

contract SimpleExtension {
    storage: ExtensionStorage
    incomingMessages: AllowedInternalMessages
    incomingExternal: AllowedExternalMessages
}

type AllowedInternalMessages =
    | TopUp
    | CancelSubscription
    | ReceivePaymentFromWallet

type AllowedExternalMessages =
    | CollectPaymentFromWallet
    | WithdrawExtensionBalance

const MIN_PLUGIN_AMOUNT = ton("0.5")

fun onExternalMessage(inMsg: slice) {
    val msg = lazy AllowedExternalMessages.fromSlice(inMsg);

    match (msg) {
        CollectPaymentFromWallet => {
            val storage = lazy ExtensionStorage.load();

            // either its the first payment or enough time has passed
            assert (
                storage.lastPaymentTime == null ||
                storage.lastPaymentTime + storage.paymentTimeInterval <
                blockchain.now()
            ) throw Errors.PaymentIntervalNotPassed;

            acceptExternalMessage();

            val w5SendSubscriptionAmountMsg = createMessage({
                body: ReceivePaymentFromWallet {},
                bounce: false,
                dest: contract.getAddress(),
                value: storage.subscriptionAmount,
            });
            val outActions = createSingleWalletSendAction(w5SendSubscriptionAmountMsg.messageCell);

            val collectPaymentMsg = createMessage({
                body: W5ExtensionActionRequest {
                    queryId: msg.queryId,
                    outActions,
                    hasExtraActions: false,
                    extraActions: createEmptySlice(),
                },
                bounce: false,
                dest: storage.walletAddress,
                value: ton("0.05"),
            });
            collectPaymentMsg.send(SEND_MODE_REGULAR);
        }
        WithdrawExtensionBalance => {
            val storage = lazy ExtensionStorage.load();

            assert (
                contract.getOriginalBalance() > storage.subscriptionAmount + MIN_PLUGIN_AMOUNT
            ) throw Errors.NotEnoughExtensionBalance;

            acceptExternalMessage();

            val withdrawMsg = createMessage({
                dest: storage.admin,
                bounce: false,
                value: contract.getOriginalBalance() - MIN_PLUGIN_AMOUNT,
            });
            withdrawMsg.send(SEND_MODE_REGULAR);
        }
        else => {
            throw Errors.UnsupportedExternalMessage;
        }
    }
}

fun onInternalMessage(in: InMessage) {
    val msg = lazy AllowedInternalMessages.fromSlice(in.body);

    match (msg) {
        CancelSubscription => {
            val storage = lazy ExtensionStorage.load();

            assert (storage.admin == in.senderAddress) throw Errors.OnlyAdmin;

            val removeExtensionMsg = createMessage({
                body: W5ExtensionActionRequest {
                    queryId: msg.queryId,
                    outActions: null,
                    hasExtraActions: true,
                    extraActions: W5RemoveExtensionExtraAction {
                        addr: contract.getAddress(),
                    }.toCell()
                        .beginParse(),
                },
                bounce: false,
                dest: storage.walletAddress,
                value: 0,
            });

            removeExtensionMsg.send(SEND_MODE_CARRY_ALL_REMAINING_MESSAGE_VALUE);
        }
        ReceivePaymentFromWallet => {
            var storage = lazy ExtensionStorage.load();

            assert (storage.walletAddress == in.senderAddress) throw Errors.OnlyWallet;

            storage.lastPaymentTime = blockchain.now();
            storage.save();
        }
        TopUp => {
            // simply accept payment
        }
        else => {}
    }
}

get fun extensionInfo(): ExtensionStorage {
    return ExtensionStorage.load();
}
