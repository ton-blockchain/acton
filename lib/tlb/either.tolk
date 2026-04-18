/// Represents a TL-B `Either X Y`: either `X` (left) or `Y` (right).
///
/// Alias semantics:
/// - `TlbEither<X, Y>` is exactly `TlbEitherLeft<X> | TlbEitherRight<Y>`.
///
/// Encoding semantics:
/// - Left branch starts with tag bit `0` (`0b0`).
/// - Right branch starts with tag bit `1` (`0b1`).
///
/// Typical consumption pattern:
/// ```tolk
/// fun branchScore(choice: TlbEither<uint32, uint32>): int {
///     return match (choice) {
///         TlbEitherLeft => 1000 + choice.value,
///         TlbEitherRight => 2000 + choice.value,
///     };
/// }
/// ```
type TlbEither<X, Y> = TlbEitherLeft<X> | TlbEitherRight<Y>

/// Left constructor.
/// Encodes with tag bit `0` followed by `value`.
@inline
fun TlbEither<X, Y>.left(value: X): TlbEitherLeft<X> {
    return TlbEitherLeft { value };
}

/// Right constructor.
/// Encodes with tag bit `1` followed by `value`.
@inline
fun TlbEither<X, Y>.right(value: Y): TlbEitherRight<Y> {
    return TlbEitherRight { value };
}

/// Left variant with explicit TLB tag `0b0`.
struct (0b0) TlbEitherLeft<X> {
    value: X
}

/// Right variant with explicit TLB tag `0b1`.
struct (0b1) TlbEitherRight<X> {
    value: X
}
