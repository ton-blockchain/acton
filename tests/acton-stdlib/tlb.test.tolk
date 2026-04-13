import "@local-acton/testing/expect"
import "@local-acton/tlb/either"
import "@local-acton/tlb/maybe"

fun branchScore(choice: Either<int, int>): int {
    return match (choice) {
        EitherLeft => 1000 + choice.value,
        EitherRight => 2000 + choice.value,
    };
}

get fun `test maybe some none and unwrap or`() {
    val someValue = Maybe<int>.some(42);
    val noneValue = Maybe<int>.none();

    expect(someValue.unwrap()).toEqual(42);
    expect(someValue.unwrapOr(10)).toEqual(42);
    expect(noneValue.unwrapOr(10)).toEqual(10);
}

@test({ fail_with: 7 })
get fun `test maybe unwrap none fails`() {
    val noneValue = Maybe<int>.none();
    noneValue.unwrap();
}

get fun `test either left and right constructors`() {
    expect(branchScore(Either<int, int>.left(7))).toEqual(1007);
    expect(branchScore(Either<int, int>.right(8))).toEqual(2008);
}
