/// Tagged union that stores either `X` (left) or `Y` (right).
///
/// Alias semantics:
/// - `Either<X, Y>` is exactly `EitherLeft<X> | EitherRight<Y>`.
///
/// Encoding semantics:
/// - Left branch starts with tag bit `0` (`0b0`).
/// - Right branch starts with tag bit `1` (`0b1`).
///
/// Typical consumption pattern:
/// ```tolk
/// fun branchScore(choice: Either<uint32, uint32>): int {
///     return match (choice) {
///         EitherLeft => 1000 + choice.value,
///         EitherRight => 2000 + choice.value,
///     };
/// }
/// ```
type Either<X, Y> = EitherLeft<X> | EitherRight<Y>

/// Left constructor.
/// Encodes with tag bit `0` followed by `value`.
@inline
fun Either<X, Y>.left(value: X) {
    return EitherLeft { value };
}

/// Right constructor.
/// Encodes with tag bit `1` followed by `value`.
@inline
fun Either<X, Y>.right(value: Y) {
    return EitherRight { value };
}

/// Left variant with explicit TLB tag `0b0`.
struct (0b0) EitherLeft<X> {
    value: X
}

/// Right variant with explicit TLB tag `0b1`.
struct (0b1) EitherRight<X> {
    value: X
}
