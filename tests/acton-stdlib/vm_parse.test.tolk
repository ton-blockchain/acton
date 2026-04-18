import "@local-acton/fmt"
import "@local-acton/testing/expect"

get fun `test parseInt with positive number`() {
    val n = parseInt("12345");
    expect(n).toEqual(12345);
}

get fun `test parseInt with negative number`() {
    val n = parseInt("-42");
    expect(n).toEqual(-42);
}
