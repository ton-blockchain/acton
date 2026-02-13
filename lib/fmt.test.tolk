import "fmt"
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
    val formatted = format1("{}", VeryNested { });
    expect(formatted).toEqual("""VeryNested {
    a: 3,
    nested: VeryNested1 {
        a: 4,
        nested: DoubleInt {
            a: 0,
            b: 1
        }
    }
}""");
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
        var loaded = inMsg.value.load();
        loaded.info.createdLt = 0;
        loaded.info.createdAt = 0;
        expect(format1("{}", loaded)).toEqual("""MessageRelaxedGeneric {
    info: IntMsgInfoRelaxed {
        ihrDisabled: true,
        bounce: false,
        bounced: false,
        src: kQAKIOo5BYX6HLXvPp8Ct55dApFwnI0qWrx55yyF/aI9Kyw0,
        dest: kQCaMdGt9cG70iD0tRb+8Dvt8QqRPDgRrLFHP4wanVjLn9No (counter),
        value: CurrencyCollection {
            grams: 1999900000,
            other: null
        },
        extraFlags: 0,
        fwdFee: 266669,
        createdLt: 0,
        createdAt: 0
    },
    init: null,
    body: null
}""");
    }
}
