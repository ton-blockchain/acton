import "@local-acton/fmt"
import "@local-acton/emulation/network"
import "@local-acton/testing/expect"
import "@local-acton/tlb/maybe"
import "@local-acton/types/message"

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

get fun `test format struct with single field`() {
    expect(format("{}", SingleInt { a: 100 })).toEqual("""SingleInt {
    a: 100,
}""");
}

get fun `test format struct with two fields`() {
    expect(
        format(
            "{}",
            DoubleInt {
                a: 100,
                b: 200,
            },
        ),
    ).toEqual("""DoubleInt {
    a: 100,
    b: 200,
}""");
}

get fun `test format struct with nested single int`() {
    expect(
        format(
            "{}",
            DoubleIntAndNestedSingleInt {
                a: 100,
                b: 200,
                nested: SingleInt { a: 300 },
            },
        ),
    ).toEqual("""DoubleIntAndNestedSingleInt {
    a: 100,
    b: 200,
    nested: SingleInt {
        a: 300,
    },
}""");
}

get fun `test format struct with nested double int`() {
    expect(
        format("{}", DoubleIntAndNestedDoubleInt {}),
    ).toEqual("""DoubleIntAndNestedDoubleInt {
    a: 3,
    b: 4,
    nested: DoubleInt {
        a: 0,
        b: 1,
    },
}""");
}

get fun `test format struct with very nested`() {
    val formatted = format("{}", VeryNested {});
    expect(
        formatted,
    ).toEqual("""VeryNested {
    a: 3,
    nested: VeryNested1 {
        a: 4,
        nested: DoubleInt {
            a: 0,
            b: 1,
        },
    },
}""");
}

get fun `test format message`() {
    val src = randomAddress("src");
    val dest = randomAddress("counter");
    val msg = createMessage({
        bounce: false,
        value: ton("1.9999"),
        dest,
    });
    val result = net.send(src, msg);
    val first = result.at(0);
    val tx = first.tx.load();

    val inMsg = tx.messages.load().inMsg;
    if (inMsg is TlbJust<Cell<TlbMessageRelaxedGeneric>>) {
        var loaded = inMsg.value.load();
        loaded.info.createdLt = 0;
        loaded.info.createdAt = 0;
        // todo "union with unresolved layout" below, to be fixed later (need to reconsider instantated generics, now they do not carry stack layout)
        expect(
            format("{}", loaded),
        ).toEqual("""TlbMessageRelaxedGeneric {
    info: TlbIntMsgInfoRelaxed {
        ihrDisabled: true,
        bounce: false,
        bounced: false,
        src: kQC5PHBsPEHzypAUMCGXCD5pNDAA2DozF5CZxqKiB1_dtgb3 (src),
        dest: kQC0663BO7-gCwST4bHolxt85JLRMaFVZgJfblE7oumFduR9 (counter),
        value: TlbCurrencyCollection {
            grams: 1999900000,
            other: map<int32, varuint32> {},
        },
        extraFlags: 0,
        fwdFee: 266669,
        createdLt: 0,
        createdAt: 0,
    },
    init: union with unresolved layout,
    body: null,
}""");
    }
}

get fun `test format multiple arguments and modifiers`() {
    expect(format("{} {:x}", "hex", 255)).toEqual("hex ff");
    expect(format("{} {} {:ton}", "value", 7, ton("1.5"))).toEqual("value 7 1.5 TON");
    expect(format("{{{}}} {} {} {}", 1, 2, 3, 4)).toEqual("{1} 2 3 4");
    expect(
        format(
            "{} {} {} {} {}",
            1,
            2,
            3,
            4,
            5,
        ),
    ).toEqual("1 2 3 4 5");
}

get fun `test format with optional arguments`() {
    expect(format("hello")).toEqual("hello");
    expect(format("value = {}", 42)).toEqual("value = 42");
    expect(format("pair {} {}", 0, 0)).toEqual("pair 0 0");
    expect(
        format(
            "{} {} {} {} {}",
            1,
            2,
            3,
            4,
            5,
        ),
    ).toEqual("1 2 3 4 5");
}
