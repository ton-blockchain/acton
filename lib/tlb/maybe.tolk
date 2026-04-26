/// Represents an optional value (TL-B Maybe) that may or may not be present.
/// This is similar to the Option type in other programming languages.
type TlbMaybe<X> = TlbNone | TlbJust<X>

/// Represents the absence of a value.
struct (0b0) TlbNone

/// Represents a present value of type X.
struct (0b1) TlbJust<X> {
    /// The contained value.
    value: X
}

/// Creates a TL-B Maybe instance with no value (`TlbNone`).
///
/// Example:
/// ```tolk
/// val empty = TlbMaybe<slice>.none();
/// ```
fun TlbMaybe<X>.none(): TlbMaybe<X> {
    return TlbNone {};
}

/// Creates a TL-B Maybe instance with the given value.
///
/// Example:
/// ```tolk
/// val present = TlbMaybe.just(42);
/// ```
fun TlbMaybe<X>.just(value: X): TlbMaybe<X> {
    return TlbJust { value };
}

/// Unwraps the Maybe, returning the contained value if present,
/// or throws an exception if `TlbNone`.
///
/// This is unsafe. Use it only when you are certain the value is present.
/// For safe access, use `unwrapOr` instead.
///
/// # Throws
///
/// - Throws exit code `7` when called on `TlbNone`.
///
/// Example:
/// ```tolk
/// val maybeValue = TlbMaybe.just(42);
/// val value = maybeValue.unwrap(); // Returns 42
/// ```
fun TlbMaybe<X>.unwrap(self): X {
    if (self is TlbNone) {
        throw 7;
    }
    return self.value;
}

/// Unwraps the Maybe, returning the contained value if present,
/// or the provided default value if `TlbNone`.
///
/// This is the safe way to access optional values.
///
/// Example:
/// ```tolk
/// val maybeValue = TlbMaybe<string>.none();
/// val name = maybeValue.unwrapOr("default"); // Returns "default"
/// ```
fun TlbMaybe<X>.unwrapOr<U>(self, default: U): X | U {
    if (self is TlbNone) {
        return default;
    }
    return self.value;
}
