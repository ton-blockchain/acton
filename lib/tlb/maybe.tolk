struct (0b0) None

struct (0b1) Value<X> {
    value: X
}

type Maybe<X> = Value<X> | None

fun Maybe<X>.none(): Maybe<X> {
    return None{}
}

fun Maybe<X>.some(value: X): Maybe<X> {
    return Value { value }
}

fun Maybe<X>.unwrap(self): X {
    if (self is None) {
        throw 7;
    }
    return self.value;
}
