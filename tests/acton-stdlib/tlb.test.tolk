import "@local-acton/testing/expect"
import "@local-acton/tlb/either"
import "@local-acton/tlb/maybe"

fun branchScore(choice: TlbEither<int, int>): int {
    return match (choice) {
        TlbEitherLeft => 1000 + choice.value,
        TlbEitherRight => 2000 + choice.value,
    };
}

get fun `test maybe some none and unwrap or`() {
    val someValue: TlbMaybe<int> = TlbMaybe.just(42);
    val noneValue: TlbMaybe<int> = TlbMaybe.none();

    expect(someValue.unwrap()).toEqual(42);
    expect(someValue.unwrapOr(10)).toEqual(42);
    expect(noneValue.unwrapOr(10)).toEqual(10);
}

@test.fail_with(7)
get fun `test maybe unwrap none fails`() {
    val noneValue = TlbMaybe<int>.none();
    noneValue.unwrap();
}

get fun `test either left and right constructors`() {
    expect(branchScore(TlbEither<int, int>.left(7))).toEqual(1007);
    expect(branchScore(TlbEither<int, int>.right(8))).toEqual(2008);
}
