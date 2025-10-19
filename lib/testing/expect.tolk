const ASSERTION_FAILED = 567

struct Assert

fun Assert.equal<V1, V2>(left: V1, right: V2, message: slice = "", location: slice = "") {
    if (!assert_equal(
        __typeName(left), left.__toTuple(),
        __typeName(right), right.__toTuple(),
        message, location)
    ) {
        throw ASSERTION_FAILED;
    }
}

fun Assert.notEqual<V1, V2>(left: V1, right: V2, message: slice = "", location: slice = "") {
    if (assert_equal(
        __typeName(left), left.__toTuple(),
        __typeName(right), right.__toTuple(),
        message, location)
    ) {
        throw ASSERTION_FAILED;
    }
}

fun Assert.fail(message: slice = "", location: slice = "") {
    assert_fail(message, location);
    throw ASSERTION_FAILED;
}

fun assert_equal<V1, V2>(left_name: slice, left: V1, right_name: slice, right: V2, message: slice = "", location: slice = ""): bool asm "4 EXTCALL"

fun assert_fail(message: slice = "", location: slice = ""): bool asm "100 EXTCALL"

struct Expectation<T> {
    private value: T
    private value_type: slice
    private location: slice = ""
}

fun Expectation<T>.create(value: T, value_type: slice, location: slice = "") {
    return Expectation<T> {
        value, value_type, location
    }
}

fun Expectation<T>.toEqual<U>(self, expected: U) {
    Assert.equal(self.value, expected, "expect(actual).toEqual(expected)", self.location)
}

fun Expectation<T>.toNotEqual<U>(self, expected: U) {
    Assert.notEqual(self.value, expected, "expect(actual).toNotEqual(expected)", self.location)
}

fun Expectation<T>.isNull(self) {
    Assert.equal(self.value, null, "expect(actual).isNull()", self.location)
}

fun Expectation<T>.isNotNull(self) {
    Assert.notEqual(self.value, null, "expect(actual).isNotNull()", self.location)
}

fun expect<T>(value: T, location: slice = "") {
    return Expectation<T>.create(value, __typeName(value), location)
}
