/// Represents an optional value that may or may not be present.
/// This is similar to the Option type in other programming languages.
type Maybe<X> = Value<X> | None

/// Represents the absence of a value.
struct (0b0) None

/// Represents a present value of type X.
struct (0b1) Value<X> {
    /// The contained value.
    value: X
}

/// Creates a Maybe instance with no value (None).
///
/// Example:
/// ```tolk
/// val empty = Maybe<slice>.none();
/// ```
fun Maybe<X>.none(): Maybe<X> {
    return None{}
}

/// Creates a Maybe instance with the given value.
///
/// Example:
/// ```tolk
/// val present = Maybe.some(42);
/// ```
fun Maybe<X>.some(value: X): Maybe<X> {
    return Value { value }
}

/// Unwraps the Maybe, returning the contained value if present,
/// or throws an exception if None.
///
/// This is unsafe — use only when you're certain the value is present.
/// For safe access, use `unwrapOr` instead.
///
/// # Throws
///
/// - Throws exit code `7` when called on `None`.
///
/// Example:
/// ```tolk
/// val maybeValue = Maybe.some(42);
/// val value = maybeValue.unwrap(); // Returns 42
/// ```
fun Maybe<X>.unwrap(self): X {
    if (self is None) {
        throw 7;
    }
    return self.value;
}

/// Unwraps the Maybe, returning the contained value if present,
/// or the provided default value if None.
///
/// This is the safe way to access optional values.
///
/// Example:
/// ```tolk
/// val maybeValue = Maybe<slice>.none();
/// val name = maybeValue.unwrapOr("default"); // Returns "default"
/// ```
fun Maybe<X>.unwrapOr(self, default: X): X {
    if (self is None) {
        return default
    }
    return self.value;
}
