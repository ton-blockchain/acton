import "methods"
import "../testing/transaction_expect"

const CANNOT_RUN_GET_METHOD_OD_UNDEPLOYED_CONTRACT = 678
const CANNOT_RUN_GET_METHOD_OF_CONTRACT_WITHOUT_CODE = 679

fun accuireNet(id: slice): Network {
    return Network{};
}

// Dummy struct for now
struct Network

fun Network.create(): Network {
    return Network{}
}

struct SendParams {
    value: int
}

fun Network.sendFromNull(self, message: OutMessage, mode: int): TransactionList {
    return sendMessage(message.messageCell, mode)!
}

fun Network.send(self, from: address, message: OutMessage, mode: int): TransactionList {
    return sendMessageFrom(message.messageCell, from.toCell(), mode)!
}

struct Treasury {
    address: address
}

fun Network.treasury(self, name: slice): Treasury {
    val state_init: ContractState = {
        code: treasuryCode(),
        data: beginCell().storeSlice(name).endCell(),
    };
    val address = AutoDeployAddress {
        stateInit: state_init,
    }.buildAddress().endCell().beginParse().loadAddress();
    val msg = createMessage({
        bounce: false,
        value: ton("10000"),
        dest: {
            workchain: 0,
            stateInit: state_init,
        },
    });
    // Deploy treasury
    self.sendFromNull(msg, SEND_MODE_PAY_FEES_SEPARATELY);
    return Treasury { address }
}

fun Network.runGetMethod<Ret, Args = tuple>(self, addr: address, name: slice, args: Args? = null): Ret {
    val method_id = (crc16(name) & 0xffff) | 0x10000;
    return self.runGetMethodById(addr, method_id, args)
}

fun Network.runGetMethodById<Ret, Args = tuple>(self, addr: address, id: int, args: Args? = null): Ret {
    if (!self.isDeployed(addr)) {
        throw CANNOT_RUN_GET_METHOD_OD_UNDEPLOYED_CONTRACT;
    }

    val code = self.getDeployedCode(addr);
    if (code == null) {
        throw CANNOT_RUN_GET_METHOD_OF_CONTRACT_WITHOUT_CODE;
    }

    val args_to_send = args == null ? createEmptyTuple() : args.__toTuple();
    val res = runGetMethod(addr.toCell(), code, id, Ret.__typeName2(), args_to_send);
    return Ret.fromTuple(res)
}

fun Network.isDeployed(self, addr: address): bool {
    return isDeployed(addr.toCell())
}

fun Network.getDeployedCode(self, addr: address): cell? {
    return getDeployedCode(addr.toCell())
}

fun treasuryCode(): cell asm """
    "te6cckEBBAEARQABFP8A9KQT9LzyyAsBAgEgAwIAWvLT/+1E0NP/0RK68qL0BNH4AH+OFiGAEPR4b6UgmALTB9QwAfsAkTLiAbPmWwAE0jD+omUe" base64>B B>boc PUSHREF
"""

fun isDeployed(addr: cell): bool asm "11 EXTCALL"
fun getDeployedCode(addr: cell): cell? asm "12 EXTCALL"
fun crc16(data: slice): int asm "13 EXTCALL"

