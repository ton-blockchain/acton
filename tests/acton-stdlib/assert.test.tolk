import "@local-acton/testing/assert"
import "@local-acton/testing/expect"

get fun `test consume less with lambda`() {
    Assert.consumesLessThan<int>(
        fun(a: int) { return fun() {
            return a + 10;
        } }(random.range(100)),
        800,
    );

    Assert.consumesLessThan<int>(
        fun(a: int, b: int) { return fun() {
            return a + b;
        } }(random.range(100), random.range(100)),
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

    var (a, b, c) = ([10].get(0), [20].get(0), [30].get(0));
    val sum = Assert.consumesLessThan(
        fun() {
            return a + b + c;
        },
        900,
    );
    expect(sum).toEqual(60);
}

get fun `test assert basic helpers`() {
    Assert.equal(10, 10);
    Assert.notEqual(10, 20);
    Assert.equalDecimal(ton("1.5"), ton("1.5"), 9);
}
