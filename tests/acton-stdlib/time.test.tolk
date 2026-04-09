import "@local-acton/emulation/network"
import "@local-acton/testing/expect"

/// After net.setNow(), blockchain.now() must return the same value as net.now().
get fun `test setNow syncs blockchain now`() {
    net.setNow(1000000);
    expect(blockchain.now()).toEqual(1000000);
    expect(net.now()).toEqual(1000000);

    net.setNow(2000000);
    expect(blockchain.now()).toEqual(2000000);
    expect(net.now()).toEqual(2000000);
}

/// Multiple setNow calls should always keep blockchain.now() and net.now() in sync.
get fun `test setNow consistency across multiple calls`() {
    var t: uint32 = 1000;
    while (t <= 5000) {
        net.setNow(t);
        expect(blockchain.now()).toEqual(t);
        expect(net.now()).toEqual(t);
        t += 1000;
    }
}
