import "@local-acton/vm/vm"
import "@local-acton/testing/expect"

get fun `test parseInt with positive number`() {
    val n = vm.parseInt("12345");
    expect(n).toEqual(12345);
}

get fun `test parseInt with negative number`() {
    val n = vm.parseInt("-42");
    expect(n).toEqual(-42);
}
