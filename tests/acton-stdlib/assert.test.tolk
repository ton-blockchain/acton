import "@local-acton/testing/assert"
import "@local-acton/testing/expect"

get fun `test consume less with lambda`() {
    Assert.consumesLessThan1(
        fun(a: int) {
            return a + 10;
        },
        random.range(100),
        800,
    );

    Assert.consumesLessThan2(
        fun(a: int, b: int) {
            return a + b;
        },
        random.range(100),
        random.range(100),
        700,
    );
}

get fun `test consume less without args and with three args`() {
    val result = Assert.consumesLessThan(
        fun() {
            return 42;
        },
        750,
    );
    expect(result).toEqual(42);

    val sum = Assert.consumesLessThan3(
        fun(a: int, b: int, c: int) {
            return a + b + c;
        },
        10,
        20,
        30,
        900,
    );
    expect(sum).toEqual(60);
}

get fun `test assert basic helpers`() {
    Assert.equal(10, 10);
    Assert.notEqual(10, 20);
    Assert.equalDecimal(ton("1.5"), ton("1.5"), 9);
}
