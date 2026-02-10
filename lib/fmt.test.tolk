import "fmt"
import "fs"
import "emulation/network"
import "testing/expect"
import "tlb/maybe"
import "types/message"

struct SingleInt {
    a: int
}

struct DoubleInt {
    a: int = 0
    b: int = 1
}

struct DoubleIntAndNestedSingleInt {
    a: int
    b: int
    nested: SingleInt
}

struct DoubleIntAndNestedDoubleInt {
    a: int = 3
    b: int = 4
    nested: DoubleInt = {}
}

struct VeryNested {
    a: int = 3
    nested: VeryNested1 = {}
}

struct VeryNested1 {
    a: int = 4
    nested: DoubleInt = {}
}

get fun `test-format-struct-with-single-field`() {
    expect(format1("{}", SingleInt { a: 100 })).toEqual("""SingleInt {
    a: 100
}""");
}

get fun `test-format-struct-with-two-fields`() {
    expect(format1("{}", DoubleInt { a: 100, b: 200 })).toEqual("""DoubleInt {
    a: 100,
    b: 200
}""");
}

get fun `test-format-struct-with-nested-single-int`() {
    expect(format1("{}", DoubleIntAndNestedSingleInt { a: 100, b: 200, nested: SingleInt { a: 300 } })).toEqual("""DoubleIntAndNestedSingleInt {
    a: 100,
    b: 200,
    nested: SingleInt {
        a: 300
    }
}""");
}

get fun `test-format-struct-with-nested-double-int`() {
    expect(format1("{}", DoubleIntAndNestedDoubleInt {})).toEqual("""DoubleIntAndNestedDoubleInt {
    a: 3,
    b: 4,
    nested: DoubleInt {
        a: 0,
        b: 1
    }
}""");
}

get fun `test-format-struct-with-very-nested-`() {
    val expectedType = fs.readFile("lib/testdata/nested_type_fmt.txt");
    expect(format1("{}", VeryNested {})).toEqual(expectedType);
}

get fun `test-format-message`() {
    val src = net.randomAddress("src");
    val dest = net.randomAddress("counter");
    val msg = createMessage({
        bounce: false,
        value: ton("1.9999"),
        dest,
    });
    val result = net.send(src, msg, SEND_MODE_REGULAR);
    val first = result.at(0);
    val tx = first.tx.load();

    val inMsg = tx.messages.load().inMsg;
    if (inMsg is Value<Cell<MessageRelaxedGeneric>>) {
        val expectedType = fs.readFile("lib/testdata/message_type_fmt.txt")!;
        expect(format1("{}", inMsg.value.load())).toEqual(expectedType);
    }
}
