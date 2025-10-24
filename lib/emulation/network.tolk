import "../build/build"
import "../ffi/ffi"
import "../types/transaction"

const CANNOT_RUN_GET_METHOD_OD_UNDEPLOYED_CONTRACT = 678
const CANNOT_RUN_GET_METHOD_OF_CONTRACT_WITHOUT_CODE = 679

/// Abstraction representing a blockchain and methods of interacting with it.
struct net

/// Represents a set of parameters for creating a message.
struct SendParams {
    /// Value to send with the message.
    value: coins
}

/// Internal method, don't use it directly.
fun net.sendFromNull(message: OutMessage, mode: int): TransactionList {
    return ffi.sendMessage(message.messageCell, mode)!
}

/// Send a message from a specified address with specified mode.
/// 
/// Example:
/// ```
/// get fun `test-send-message`() {
///     // ...
///     val message = createMessage({
///         bounce: false,
///         dest: counter.address,
///         value: ton("1"),
///     });
///     net.send(deployer.address, message, SEND_MODE_PAY_FEES_SEPARATELY);
/// }
/// ```
fun net.send(from: address, message: OutMessage, mode: int): TransactionList {
    return ffi.sendMessageFrom(message.messageCell, from.toCell(), mode)!
}

/// Treasury is a special contract that is used to deploy other contracts.
/// 
/// To create a treasury, use the `Network.treasury()` method.
struct Treasury {
    address: address
}

/// Create a treasury with a specified name.
/// 
/// Calling this method with same name will return the same treasury.
/// 
/// Example:
/// ```
/// get fun `test-mint`() {
///     val net = accuireNet("net");
///     val treasury = net.treasury("treasury");
///     // ...
/// }
/// ```
fun net.treasury(name: slice): Treasury {
    val state_init: ContractState = {
        code: __treasuryCode(),
        data: beginCell().storeSlice(name).endCell(),
    };

    val address = address.fromValidBuilder(AutoDeployAddress { stateInit: state_init }.buildAddress());

    val msg = createMessage({
        bounce: false,
        value: ton("10000"),
        dest: {
            workchain: 0,
            stateInit: state_init,
        },
    });
    // Deploy treasury
    net.sendFromNull(msg, SEND_MODE_PAY_FEES_SEPARATELY);
    return Treasury { address }
}

/// Run a get method of a contract by its name.
/// 
/// Imagine you have the following get method in the contract:
/// ```
/// get fun get_balance(): coins { ... }
/// ```
/// 
/// To call it, you can use the following code:
/// ```
/// val balance = net.runGetMethod<coins>(wallet.address, "get_balance");
/// ```
/// Since this get method doesn't accept any arguments, third argument can be omitted.
///
/// ## With argument
///
/// If the get method accepts arguments, you can pass them as a third argument.
/// ```
/// get fun get_wallet_address(ownerAddress: address): address { ... }
/// ```
/// ```
/// val address = net.runGetMethod<address>(wallet.address, "get_wallet_address", deployer.address);
/// ```
///
/// ## With struct return type
/// 
/// If get method returns a struct, just specify the type of the struct as a first type argument:
/// ```
/// get fun get_jetton_data(): JettonDataReply { ... }
/// ```
/// ```
/// val data = net.runGetMethod<JettonDataReply>(wallet.address, "get_jetton_data");
/// ```
/// 
/// If you need to run a get method by its ID, use [Network.runGetMethodById] instead.
fun net.runGetMethod<Ret, Args = tuple>(addr: address, name: slice, args: Args? = null): Ret {
    val method_id = (crc16(name) & 0xffff) | 0x10000;
    return net.runGetMethodById(addr, method_id, args)
}

/// Run a get method of a contract by its ID.
/// 
/// If you need to run a get method by its name, use [Network.runGetMethod] instead.
/// 
/// Example:
/// ```
/// val data = net.runGetMethodById<JettonDataReply>(wallet.address, 0x17b02);
/// ```
/// 
/// See [Network.runGetMethod] for more information.
fun net.runGetMethodById<Ret, Args = tuple>(addr: address, id: int, args: Args? = null): Ret {
    if (!net.isDeployed(addr)) {
        throw CANNOT_RUN_GET_METHOD_OD_UNDEPLOYED_CONTRACT;
    }

    val code = net.getDeployedCode(addr);
    if (code == null) {
        throw CANNOT_RUN_GET_METHOD_OF_CONTRACT_WITHOUT_CODE;
    }

    val args_to_send = args == null ? createEmptyTuple() : args.__toTuple();
    val res = ffi.runGetMethod(addr.toCell(), code, id, Ret.__typeName2(), args_to_send);
    return Ret.fromTuple(res)
}

/// Check if a contract with a specified address is deployed.
fun net.isDeployed(addr: address): bool {
    return ffi.isDeployed(addr.toCell())
}

/// Get the code of a deployed contract by its address.
/// 
/// If the contract is not deployed, returns null.
fun net.getDeployedCode(addr: address): cell? {
    return ffi.getDeployedCode(addr.toCell())
}

/// Calculate the CRC16 checksum of a slice.
/// 
/// Example:
/// ```
/// val crc = crc16("hello");
/// ```
fun crc16(data: slice): int asm "13 EXTCALL"

fun address.fromTuple(data: tuple): address  asm "1 UNTUPLE"
fun int.fromTuple(data: tuple): int  asm "1 UNTUPLE"
fun coins.fromTuple(data: tuple): coins  asm "1 UNTUPLE"
fun bool.fromTuple(data: tuple): bool  asm "1 UNTUPLE"
fun slice.fromTuple(data: tuple): slice  asm "1 UNTUPLE"
fun cell.fromTuple(data: tuple): cell  asm "1 UNTUPLE"

fun __treasuryCode(): cell asm """
    "te6cckEBBAEARQABFP8A9KQT9LzyyAsBAgEgAwIAWvLT/+1E0NP/0RK68qL0BNH4AH+OFiGAEPR4b6UgmALTB9QwAfsAkTLiAbPmWwAE0jD+omUe" base64>B B>boc PUSHREF
"""

fun ffi.sendMessage(msg: cell, mode: int): TransactionList? asm "7 EXTCALL"
fun ffi.sendMessageFrom(msg: cell, from: cell, mode: int): TransactionList? asm "9 EXTCALL"
fun ffi.runGetMethod(addr: cell, code: cell, id: int, return_type: slice, args: tuple): tuple asm "8 EXTCALL"
fun ffi.isDeployed(addr: cell): bool asm "11 EXTCALL"
fun ffi.getDeployedCode(addr: cell): cell? asm "12 EXTCALL"
