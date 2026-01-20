type Either<X, Y> = EitherLeft<X> | EitherRight<Y>

/// Left contructor
@inline
fun Either<X, Y>.left(value: X) {
    return EitherLeft { value };
}

/// Right contructor
@inline
fun Either<X, Y>.right(value: Y) {
    return EitherRight { value };
}

struct (0b0) EitherLeft<X> {
    value: X
}

struct (0b1) EitherRight<X> {
    value: X
}
