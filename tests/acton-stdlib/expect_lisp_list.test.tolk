import "@local-acton/testing/expect"

fun filledList(): lisp_list<int> {
    return [1, 2, 3];
}

get fun `test expect lisp list matchers non empty`() {
    val values = filledList();

    expect(values).toContain(1);
    expect(values).toContain(3);
    expect(values).toNotContain(99);
    expect(values).toBeNonEmpty();
    expect(values).toHaveLength(3);
}

get fun `test expect lisp list matchers empty`() {
    val values = lisp_list<int> [];

    expect(values).toBeEmpty();
    expect(values).toNotContain(1);
    expect(values).toHaveLength(0);
}

@test.fail_with(567)
get fun `test expect lisp list to contain missing fails`() {
    expect(filledList()).toContain(99);
}

@test.fail_with(567)
get fun `test expect lisp list to not contain present fails`() {
    expect(filledList()).toNotContain(2);
}

@test.fail_with(567)
get fun `test expect lisp list to be empty on non empty fails`() {
    expect(filledList()).toBeEmpty();
}

@test.fail_with(567)
get fun `test expect lisp list to be non empty on empty fails`() {
    val values = lisp_list<int> [];
    expect(values).toBeNonEmpty();
}

@test.fail_with(567)
get fun `test expect lisp list to have length mismatch fails`() {
    expect(filledList()).toHaveLength(4);
}
