import "@local-acton/vm/vm"
import "@local-acton/testing/expect"

get fun `test parseAddress with raw address`() {
    val raw = "0:0000000000000000000000000000000000000000000000000000000000000000";
    val addr = vm.parseAddress(raw);
    val converted = vm.convertAddress(raw);
    expect(addr).toEqual(converted);
}

get fun `test parseAddress with user friendly address`() {
    val friendly = "EQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAM9c";
    val addr = vm.parseAddress(friendly);
    val raw = "0:0000000000000000000000000000000000000000000000000000000000000000";
    val expected = vm.convertAddress(raw);
    expect(addr).toEqual(expected);
}

get fun `test parseInt with positive number`() {
    val n = vm.parseInt("12345");
    expect(n).toEqual(12345);
}

get fun `test parseInt with negative number`() {
    val n = vm.parseInt("-42");
    expect(n).toEqual(-42);
}
