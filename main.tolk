import "lib/testing/expect"
import "lib/fs"
import "lib/io"

fun build(path: slice): cell asm "6 EXTCALL"

type TransactionList = tuple

fun TransactionList.at(self, i: int): Transaction {
    val raw_tx = self.get<Cell<Transaction>>(0);
    return raw_tx.load({
        assertEndAfterReading: false
    })
}

fun Expectation<TransactionList>.toHaveSuccefulTransaction(self, to: AutoDeployAddress | address) {
    val value = self.value;

    if (value.size() == 0) {
        // Need fail function
        Assert.equal(true, false);
    }

    var i = 0;
    while (i < value.size()) {
        val item = value.at(i);

        val address = beginCell().storeUint(4, 3).storeUint(0, 8).storeUint(item.address, 256).endCell();

        match (to) {
            AutoDeployAddress => {
                Assert.equal(address, to.buildAddress().endCell())
            }
            address => {
                Assert.equal(address, beginCell().storeAddress(to).endCell())
            }
        }

        i += 1;
    }
}

fun sendMessage(msg: cell, mode: int): TransactionList? asm "7 EXTCALL"

fun runGetMethod<T>(addr: cell, code: cell, id: int): T asm "8 EXTCALL"

struct Data {
    value: int32
    is_set: bool
}

get fun `test-first`() {
    expect(Data{
        value: 0, is_set: false
    }).toEqual(Data{
        value: 0, is_set: true
    });
}

get fun `test-second`() {
    Data.fromCell(contract.getData());

    // throw 9;
}

get fun `test-third`() {
    // ...
}

get fun test_fourth() {
    // ...
}

fun run_test(cb: () -> void) {
    cb()
}

struct (0b00) Uninitialized
struct (0b01) Frozen
struct (0b10) Active
struct (0b11) NonExisting

type AccountStatus = Uninitialized | Frozen | Active | NonExisting

struct Transaction {
    tag: int4 // 0x07
    address: uint256
    lt: uint64
    prevTransactionHash: uint256
    prevTransactionLt: uint64
    now: uint32
    outMessagesCount: uint15
    oldStatus: AccountStatus
    endStatus: AccountStatus
}

fun OutMessage.sendSync(self, sendMode: int): TransactionList? {
    return sendMessage(self.messageCell, sendMode)
}

fun main() {
    val cell = build("1.tolk");
    val address = AutoDeployAddress {
        workchain: 0,
        stateInit: ContractState {
            code: cell,
            data: beginCell().storeInt(5, 32).endCell(),
        }
    };

    val msg = createMessage({
        bounce: false,
        value: ton("0.1"),
        dest: address,
        body: createEmptyCell(),
    });

    val res = msg.sendSync(SEND_MODE_REGULAR);
    if (res == null) {
        eprintln("Cannot parse message");
        return;
    }

    expect(res).toHaveSuccefulTransaction(address);

    val tx = res.at(0);
    println(tx.__toTuple());

    val res2 = runGetMethod<int>(address.buildAddress().endCell(), cell, (stringCrc16("some_data") & 0xffff) | 0x10000);
    println(res2);
    val res3 = runGetMethod<tuple>(address.buildAddress().endCell(), cell, (stringCrc16("some_other_data") & 0xffff) | 0x10000);
    println(res3);

    // val res2 = msg.sendSync(SEND_MODE_REGULAR);
    // val tx2 = res2!.load({assertEndAfterReading: false});
    // println(tx2.__toTuple());


    // println(cell);
    // println(address.buildAddress().endCell());

    // val ignore = fs.read_file("hello.txt");
    // if (ignore == null) {
    //     eprintln("Cannot read file");
    //     return
    // }
    // println(ignore);
    //
    // run_test(test_first);
    // register_test("test-first");
    // register_test("test-second");
    // register_test("test-third");

    // expect(10).toEqual(30);
    expect(null).toBeNotNull();
    expect(Data{
        value: 0, is_set: false
    }).toEqual(Data{
        value: 0, is_set: true
    });

    Assert.equal(10, 20);
    Assert.equal(Data{
        value: 0, is_set: false
    }, Data{
        value: 0, is_set: false
    });
    Assert.equal(Data{
        value: 1, is_set: false
    }, Data{
        value: 0, is_set: false
    });

}
