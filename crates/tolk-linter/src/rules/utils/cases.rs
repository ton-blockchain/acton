#[inline]
const fn is_lower_or_digit(b: u8) -> bool {
    b.is_ascii_lowercase() || b.is_ascii_digit()
}

#[inline]
const fn is_upper_or_digit(b: u8) -> bool {
    b.is_ascii_uppercase() || b.is_ascii_digit()
}

#[inline]
const fn skip_leading_underscores(bytes: &[u8]) -> usize {
    let mut i = 0;
    while i < bytes.len() && bytes[i] == b'_' {
        i += 1;
    }
    i
}

pub(crate) const fn is_camel_ascii(s: &str) -> bool {
    let bytes = s.as_bytes();
    let mut i = skip_leading_underscores(bytes);
    if i >= bytes.len() {
        return false;
    }

    let b = bytes[i];
    if !b.is_ascii_lowercase() {
        return false;
    }
    i += 1;
    while i < bytes.len() && is_lower_or_digit(bytes[i]) {
        i += 1;
    }

    while i < bytes.len() {
        let b1 = bytes[i];
        if !b1.is_ascii_uppercase() {
            return false;
        }
        i += 1;

        if i >= bytes.len() || !is_lower_or_digit(bytes[i]) {
            return false;
        }
        i += 1;

        while i < bytes.len() && is_lower_or_digit(bytes[i]) {
            i += 1;
        }
    }

    true
}

pub(crate) const fn is_pascal_ascii(s: &str) -> bool {
    let bytes = s.as_bytes();
    let mut i = skip_leading_underscores(bytes);
    if i >= bytes.len() {
        return false;
    }

    // One or more segments: [A-Z][a-z0-9]+
    while i < bytes.len() {
        let b = bytes[i];
        if !b.is_ascii_uppercase() {
            return false;
        }
        i += 1;

        // Require at least one [a-z0-9]
        if i >= bytes.len() || !is_lower_or_digit(bytes[i]) {
            return false;
        }
        i += 1;

        while i < bytes.len() && is_lower_or_digit(bytes[i]) {
            i += 1;
        }
    }

    true
}

pub(crate) const fn is_screaming_snake_ascii(s: &str) -> bool {
    let bytes = s.as_bytes();
    let mut i = skip_leading_underscores(bytes);
    if i >= bytes.len() {
        return false;
    }

    if !is_upper_or_digit(bytes[i]) {
        return false;
    }
    while i < bytes.len() && is_upper_or_digit(bytes[i]) {
        i += 1;
    }

    while i < bytes.len() {
        if bytes[i] != b'_' {
            return false;
        }
        i += 1;

        // forbid trailing '_' and double '__'
        if i >= bytes.len() || !is_upper_or_digit(bytes[i]) {
            return false;
        }
        while i < bytes.len() && is_upper_or_digit(bytes[i]) {
            i += 1;
        }
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn camel_no_acronyms() {
        for s in ["foo", "foo1", "fooBar", "_fooBar9", "aB1c"] {
            assert!(is_camel_ascii(s), "{s}");
        }
        for s in [
            "",
            "_",
            "Foo",
            "FooBar",
            "foo_bar",
            "fooBAR",
            "fooHTTPServer",
            "fooB",
            "foo-бар",
        ] {
            assert!(!is_camel_ascii(s), "{s}");
        }
    }

    #[test]
    fn pascal_no_acronyms() {
        for s in ["Foo1", "FooBar2", "_FooBar9", "Ab1Cd2"] {
            assert!(is_pascal_ascii(s), "{s}");
        }
        for s in [
            "",
            "_",
            "F",
            "HTTPServer",
            "URL2JSON",
            "fooBar",
            "Foo_Bar",
            "FooBAR",
        ] {
            assert!(!is_pascal_ascii(s), "{s}");
        }
    }

    #[test]
    fn screaming_snake() {
        for s in ["FOO", "FOO_BAR", "_FOO2_BAR3", "A1_B2_C3"] {
            assert!(is_screaming_snake_ascii(s), "{s}");
        }
        for s in ["", "_", "FOO_", "FOO__BAR", "Foo_BAR", "FOO-bar", "__"] {
            assert!(!is_screaming_snake_ascii(s), "{s}");
        }
    }
}
