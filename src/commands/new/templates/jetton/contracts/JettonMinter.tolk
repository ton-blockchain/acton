import "errors"
import "fees-management"
import "jetton-utils"
import "messages"
import "storage"

contract JettonMinter {
    storage: MinterStorage
    incomingMessages: AllowedMessageToMinter
}

type AllowedMessageToMinter =
    | BurnNotificationForMinter
    | RequestWalletAddress
    | MintNewJettons
    | ChangeMinterAdmin
    | ChangeMinterContent

fun onInternalMessage(in: InMessage) {
    val msg = lazy AllowedMessageToMinter.fromSlice(in.body);

    match (msg) {
        BurnNotificationForMinter => {
            var storage = lazy MinterStorage.load();
            assert (
                in.senderAddress ==
                calcAddressOfJettonWallet(
                    msg.burnInitiator,
                    contract.getAddress(),
                    storage.jettonWalletCode,
                )
            ) throw Errors.UnauthorizedBurn;

            storage.totalSupply -= msg.jettonAmount;
            storage.save();

            if (msg.sendExcessesTo == null) {
                return;
            }

            val excessesMsg = createMessage({
                bounce: BounceMode.NoBounce,
                dest: msg.sendExcessesTo,
                value: 0,
                body: ReturnExcessesBack { queryId: msg.queryId },
            });
            excessesMsg.send(SEND_MODE_IGNORE_ERRORS + SEND_MODE_CARRY_ALL_REMAINING_MESSAGE_VALUE);
        }

        RequestWalletAddress => {
            assert (
                in.valueCoins > in.originalForwardFee + MINIMAL_MESSAGE_VALUE_BOUND
            ) throw Errors.NotEnoughAmountToRespond;

            val respondOwnerAddress: Cell<address>? = msg.includeOwnerAddress
                ? msg.ownerAddress.toCell()
                : null;

            var walletAddress: address? = null;
            if (msg.ownerAddress.getWorkchain() == BASECHAIN) {
                val storage = lazy MinterStorage.load();
                walletAddress = calcAddressOfJettonWallet(
                    msg.ownerAddress,
                    contract.getAddress(),
                    storage.jettonWalletCode,
                );
            }

            val respondMsg = createMessage({
                bounce: BounceMode.Only256BitsOfBody,
                dest: in.senderAddress,
                value: 0,
                body: ResponseWalletAddress {
                    queryId: msg.queryId,
                    jettonWalletAddress: walletAddress,
                    ownerAddress: respondOwnerAddress,
                },
            });
            respondMsg.send(SEND_MODE_CARRY_ALL_REMAINING_MESSAGE_VALUE);
        }

        MintNewJettons => {
            var storage = lazy MinterStorage.load();
            assert (in.senderAddress == storage.adminAddress) throw Errors.NotFromAdmin;

            val internalTransferMsg = lazy msg.internalTransferMsg.load();
            storage.totalSupply += internalTransferMsg.jettonAmount;
            storage.save();

            val deployMsg = createMessage({
                bounce: BounceMode.Only256BitsOfBody,
                dest: calcDeployedJettonWallet(
                    msg.mintRecipient,
                    contract.getAddress(),
                    storage.jettonWalletCode,
                ),
                value: msg.tonAmount,
                body: msg.internalTransferMsg, // a newly-deployed wallet contract will immediately handle it
            });
            deployMsg.send(SEND_MODE_PAY_FEES_SEPARATELY);
        }

        ChangeMinterAdmin => {
            var storage = lazy MinterStorage.load();
            assert (in.senderAddress == storage.adminAddress) throw Errors.NotFromAdmin;
            storage.adminAddress = msg.newAdminAddress;
            storage.save();
        }

        ChangeMinterContent => {
            var storage = lazy MinterStorage.load();
            assert (in.senderAddress == storage.adminAddress) throw Errors.NotFromAdmin;
            storage.content = msg.newContent;
            storage.save();
        }

        else => {
            // ignore empty messages, "wrong opcode" for others
            assert (in.body.isEmpty()) throw Errors.InvalidMessage;
        }
    }
}

get fun get_jetton_data(): JettonDataReply {
    val storage = lazy MinterStorage.load();

    return {
        totalSupply: storage.totalSupply,
        mintable: true,
        adminAddress: storage.adminAddress,
        jettonContent: storage.content,
        jettonWalletCode: storage.jettonWalletCode,
    };
}

get fun get_wallet_address(ownerAddress: address): address {
    val storage = lazy MinterStorage.load();
    return calcAddressOfJettonWallet(ownerAddress, contract.getAddress(), storage.jettonWalletCode);
}
