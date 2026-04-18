import "@local-acton/emulation/network"
import "@local-acton/emulation/testing"
import "@local-acton/testing/expect"

/// After testing.setNow(), blockchain.now() must return the same value as testing.getNow().
get fun `test setNow syncs blockchain now`() {
    testing.setNow(1000000);
    expect(blockchain.now()).toEqual(1000000);
    expect(testing.getNow()).toEqual(1000000);

    testing.setNow(2000000);
    expect(blockchain.now()).toEqual(2000000);
    expect(testing.getNow()).toEqual(2000000);
}

/// Multiple setNow calls should always keep blockchain.now() and testing.getNow() in sync.
get fun `test setNow consistency across multiple calls`() {
    var t: uint32 = 1000;
    while (t <= 5000) {
        testing.setNow(t);
        expect(blockchain.now()).toEqual(t);
        expect(testing.getNow()).toEqual(t);
        t += 1000;
    }
}
