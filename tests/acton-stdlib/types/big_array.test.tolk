import "@local-acton/types/big_array"
import "@local-acton/testing/expect"

fun createFilledBigArray(size: int): BigArray<int> {
    var big = BigArray<int>.createEmpty();

    var i = 0;
    while (i < size) {
        big.push(i);
        i += 1;
    }

    return big;
}

get fun `test-big-array-create-empty`() {
    val big = BigArray<int>.createEmpty();
    expect(big.size()).toEqual(0);
}

get fun `test-big-array-create-from-array`() {
    val big = BigArray<int>.createFromArray([4, 7, 9]);
    expect(big.size()).toEqual(3);
    expect(big.first()).toEqual(4);
    expect(big.last()).toEqual(9);
}

get fun `test-big-array-push-first-last-size`() {
    val big = createFilledBigArray(3);

    expect(big.size()).toEqual(3);
    expect(big.first()).toEqual(0);
    expect(big.last()).toEqual(2);
}

get fun `test-big-array-set-updates-element`() {
    var big = createFilledBigArray(3);

    big.set(777, 2);

    expect(big.size()).toEqual(3);
    expect(big.first()).toEqual(0);
    expect(big.last()).toEqual(777);
}

get fun `test-big-array-pop-updates-size-and-last`() {
    var big = createFilledBigArray(4);

    big.pop();

    expect(big.size()).toEqual(3);
    expect(big.first()).toEqual(0);
    expect(big.last()).toEqual(2);
}

get fun `test-big-array-second-bin-layout`() {
    var big = createFilledBigArray(260);

    expect(big.size()).toEqual(260);
    expect(big.first()).toEqual(0);
    expect(big.last()).toEqual(259);

    big.set(700, 255);

    var pops = 0;
    while (pops < 4) {
        big.pop();
        pops += 1;
    }

    expect(big.size()).toEqual(256);
    expect(big.last()).toEqual(700);
}
