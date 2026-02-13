import "../../lib/testing/assert"

get fun `test-consume less with lambda`() {
    Assert.consumesLessThan1(fun (a: int) {
        return a + 10
    },  random.range(100), 553);

    Assert.consumesLessThan2(fun (a: int, b: int) {
        return a + b
    },  random.range(100),random.range(100), 396);
}
