import "../build/build"
import "../ffi/ffi"
import "../fmt"
import "../testing/assert"
import "../tlb/maybe"
import "../types/message"
import "../types/out_actions"
import "../types/transaction"
import "../vm/vm"

const CANNOT_RUN_GET_METHOD_OD_UNDEPLOYED_CONTRACT = 678
const CANNOT_RUN_GET_METHOD_OF_CONTRACT_WITHOUT_CODE = 679

/// Abstraction representing a blockchain and methods of interacting with it.
struct net

/// Represents a set of parameters for creating a message.
struct SendParams {
    /// Value to send with the message.
    value: coins
}

/// Represents a list of child transactions IDs (LT) for single send result.
type ChildTxsIds = tuple

/// Represents a list of out messages for single send result.
type OutMessages = tuple

/// Returns the out message at the given index as a raw cell.
/// 
/// To get parsed message, use [OutMessages.at] method.
fun OutMessages.atCell(self, index: int): cell {
    return self.get<cell>(index);
}

/// Returns the out message at the given index as a parsed message of given type.
/// 
/// To get raw cell, use [OutMessages.atCell] method.
/// 
/// If actual message body has a different type, loading will fail.
fun OutMessages.at<Msg>(self, index: int): MessageRelaxed<Msg> {
    val cell = self.get<cell>(index) as Cell<MessageRelaxed<Msg>>;
    try {
        return cell.load();
    } catch (e) {
        if (e == 63) {
            // prefix mismatch
            val cell_generic = self.get<cell>(index) as Cell<MessageRelaxedGeneric>;
            val msg = cell_generic.load();

            val opcode = msg.loadOpcode();
            if (opcode == null) {
                throw e;
            }

            val opcode_type_name = ffi.type_name_by_opcode(opcode);

            Assert.fail(format4(
                "Out message is not a `{}` (opcode: 0x{:x}), but `{}` (opcode: 0x{:x})",
                Msg.__typeName2(),
                Msg.getDeclaredPackPrefix(),
                opcode_type_name == null ? "" : opcode_type_name,
                opcode
            ));
        }

        throw e;
    }
}

/// Represents a single send result obtained from [net.send] method.
struct SendResult {
    /// Raw cell with encoded transaction.
    tx: Cell<Transaction>
    /// List of child transactions IDs.
    /// This transactions are directly produced by this transaction.
    child_txs: ChildTxsIds
    /// Parent transaction LT that produced this transaction or null if this transaction is the root one.
    parent_lt: int?
    /// Raw cell with encoded out actions.
    /// Use [SendResult.allOutActions] to load all actions.
    out_actions: Cell<OutList>
    /// All out messages produced by this transaction.
    out_messages: OutMessages
    /// Amount of gas used by this transaction.
    gas_used: int
}

/// Returns all out actions produced by this transaction as a list.
fun SendResult.allOutActions(self): OutActionList {
    return vm.parseOutActions(self.out_actions);
}

/// Represents a list of send results obtained from [net.send] method.
/// 
/// This list represent all transactions produced by a [net.send] call.
/// 
/// To get a single send result, use [SendResultList.at] method.
type SendResultList = tuple

/// Returns the send result at the given index.
fun SendResultList.at(self, index: int): SendResult {
    val tuple = self.get<tuple>(index);
    return SendResult.fromTuple(tuple);
}

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

/// Find a transaction in the send result list by specified parameters.
/// 
/// Example:
/// ```
/// val tx = net.send(deployer.address, message, SEND_MODE_PAY_FEES_SEPARATELY);
/// val result = tx.findTransaction({ to: counter.address });
/// expect(result).toBeDefined();
/// val tx = result.unwrap();
/// ```
fun SendResultList.findTransaction(self, params: SearchParams): Maybe<Transaction> {
    val tx = ffi.findTransactionByParams(self, params.__toTuple());
    if (tx == null) {
        return None{};
    }
    val tx_cell = tx as Cell<Transaction>;
    return Value { value: tx_cell.load() };
}

/// Internal method, don't use it directly.
fun net.sendFromNull(message: OutMessage, mode: int): TransactionList {
    return ffi.sendMessage(message.messageCell, mode)!;
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
/// 
/// Returns a list of send results representing all transactions produced by this send call.
fun net.send(from: address, message: OutMessage, mode: int): SendResultList {
    return ffi.sendMessageFrom(message.messageCell, from.toCell(), mode)!;
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
    net.registerAddress(address, name);

    val msg = createMessage({
        bounce: false,
        value: ton("10000"),
        dest: { workchain: 0, stateInit: state_init },
    });
    // Deploy treasury
    net.sendFromNull(msg, SEND_MODE_PAY_FEES_SEPARATELY);
    return Treasury { address };
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
/// If get method returns a struct, specify the type of the struct as a first type argument:
/// ```
/// get fun get_jetton_data(): JettonDataReply { ... }
/// ```
/// ```
/// val data = net.runGetMethod<JettonDataReply>(wallet.address, "get_jetton_data");
/// ```
/// And provide glue method:
/// ```
/// fun JettonDataReply.fromTuple(t: tuple): JettonDataReply asm "<count fields> UNTUPLE"
/// ```
///
/// ## With tensor return type
///
/// If get method returns a tensor, specify tuple as a first type argument:
/// ```
/// get fun get_all_data(): (int, slice) { ... }
/// ```
/// ```
/// val data: tuple = net.runGetMethod(wallet.address, "get_all_data");
/// val first = data.get<int>(0);
/// val second = data.get<int>(1);
/// ```
/// 
/// If you need to run a get method by its ID, use [Network.runGetMethodById] instead.
fun net.runGetMethod<Ret, Args = tuple>(addr: address, name: slice, args: Args? = null): Ret {
    val method_id = (crc16(name) & 0xffff) | 0x10000;
    return net.runGetMethodById(addr, method_id, args);
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
@noinline
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
    return Ret.fromTuple(res);
}

/// Check if a contract with a specified address is deployed.
fun net.isDeployed(addr: address): bool {
    return ffi.isDeployed(addr.toCell());
}

/// Get the code of a deployed contract by its address.
/// 
/// If the contract is not deployed, returns null.
fun net.getDeployedCode(addr: address): cell? {
    return ffi.getDeployedCode(addr.toCell());
}

/// Generate a random address.
/// 
/// If a name is provided, the address will be registered with the given name.
/// This allows test runner to show the name instead of the address in the test output.
/// 
/// Example:
/// ```
/// val address = net.randomAddress("random_user");
/// ```
fun net.randomAddress(name: slice? = null): address {
    var builder = beginCell();
    builder.storeUint(0b10, 2);
    builder.storeUint(0b0, 1);
    builder.storeUint(0b0, 8);

    var i = 0;
    while (i < 32) {
        val num = random.range(256);
        builder.storeUint(num, 8);
        i += 1;
    }

    val address = address.fromValidBuilder(builder);
    if (name != null) {
        net.registerAddress(address, name);
    }
    return address;
}

/// Register an address with a name.
/// 
/// This allows test runner to show the name instead of the address in the test output.
/// 
/// For example, if your contract is not deployed yet, without calling this method, 
/// the test runner will show raw address in the test output. In this case, you 
/// likely want to call this method to register the address with a name.
/// 
/// Example:
/// ```
/// net.registerAddress(address, "my_address");
/// ```
fun net.registerAddress(addr: address, name: slice) {
    ffi.registerAddress(addr.toCell(), name);
}

/// Register a code cell with a name.
/// 
/// This allows test runner to show the name instead of the address in the test output
/// if the actual code of the contract is match to previously registered code.
/// 
/// Example:
/// ```
/// net.registerCodeCell(code, "my_code");
/// ```
fun net.registerCodeCell(code: cell, name: slice) {
    ffi.registerCodeCell(code, name);
}

/// Calculate the CRC16 checksum of a slice.
/// 
/// Example:
/// ```
/// val crc = crc16("hello");
/// ```
fun crc16(data: slice): int asm "13 EXTCALL"

fun address.fromTuple(data: tuple): address  asm "1 UNTUPLE"
fun address?.fromTuple(data: tuple): address  asm "1 UNTUPLE"
fun int.fromTuple(data: tuple): int  asm "1 UNTUPLE"
fun int?.fromTuple(data: tuple): int  asm "1 UNTUPLE"
fun coins.fromTuple(data: tuple): coins  asm "1 UNTUPLE"
fun coins?.fromTuple(data: tuple): coins  asm "1 UNTUPLE"
fun bool.fromTuple(data: tuple): bool  asm "1 UNTUPLE"
fun bool?.fromTuple(data: tuple): bool  asm "1 UNTUPLE"
fun slice.fromTuple(data: tuple): slice  asm "1 UNTUPLE"
fun slice?.fromTuple(data: tuple): slice  asm "1 UNTUPLE"
fun cell.fromTuple(data: tuple): cell  asm "1 UNTUPLE"
fun cell?.fromTuple(data: tuple): cell  asm "1 UNTUPLE"
fun SendResult.fromTuple(data: tuple): SendResult  asm "6 UNTUPLE"

fun tuple.fromTuple(data: tuple): tuple {
    return data;
}

fun __treasuryCode(): cell asm """
    "te6cckEBBAEARQABFP8A9KQT9LzyyAsBAgEgAwIAWvLT/+1E0NP/0RK68qL0BNH4AH+OFiGAEPR4b6UgmALTB9QwAfsAkTLiAbPmWwAE0jD+omUe" base64>B B>boc PUSHREF
"""

fun ffi.sendMessage(msg: cell, mode: int): TransactionList? asm "7 EXTCALL"
fun ffi.sendMessageFrom(msg: cell, from: cell, mode: int): SendResultList? asm "9 EXTCALL"
fun ffi.runGetMethod(addr: cell, code: cell, id: int, return_type: slice, args: tuple): tuple asm "8 EXTCALL"
fun ffi.isDeployed(addr: cell): bool asm "11 EXTCALL"
fun ffi.getDeployedCode(addr: cell): cell? asm "12 EXTCALL"
fun ffi.registerAddress(address: cell, name: slice): void asm "15 EXTCALL"
fun ffi.registerCodeCell(cell: cell, name: slice): void asm "16 EXTCALL"
fun ffi.findTransactionByParams(txs: tuple, params: tuple): cell? asm "10 EXTCALL"
fun ffi.failToFindTransactionByParams(location: slice, message: slice, txs: tuple, params: tuple): tuple? asm "103 EXTCALL"
fun ffi.failToNotFindTransactionByParams(location: slice, message: slice, txs: tuple, params: tuple): tuple? asm "104 EXTCALL"
