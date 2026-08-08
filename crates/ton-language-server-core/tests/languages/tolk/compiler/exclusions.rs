pub(crate) struct SyntaxExclusion {
    pub(crate) fixture: &'static str,
    pub(crate) reason: &'static str,
}

pub(crate) struct TypeExpectationExclusion {
    pub(crate) fixture: &'static str,
    pub(crate) line: u32,
    pub(crate) expression: &'static str,
    pub(crate) expected: &'static str,
    pub(crate) actual: &'static str,
    pub(crate) reason: &'static str,
}

pub(crate) struct ResolutionExclusion {
    pub(crate) fixture: &'static str,
    pub(crate) line: u32,
    pub(crate) character: u32,
    pub(crate) symbol: &'static str,
    pub(crate) reason: &'static str,
}

const fn syntax(fixture: &'static str) -> SyntaxExclusion {
    SyntaxExclusion {
        fixture,
        reason: "tree-sitter does not yet accept this valid compiler fixture",
    }
}

#[allow(dead_code)]
const fn unresolved(
    fixture: &'static str,
    line: u32,
    character: u32,
    symbol: &'static str,
    reason: &'static str,
) -> ResolutionExclusion {
    ResolutionExclusion {
        fixture,
        line,
        character,
        symbol,
        reason,
    }
}

pub(crate) const SYNTAX_EXCLUSIONS: &[SyntaxExclusion] = &[
    syntax("arrays-tuples-tests.tolk"),
    syntax("enums-tests.tolk"),
    syntax("lists-tests.tolk"),
    syntax("match-by-expr-tests.tolk"),
    syntax("pack-unpack-5.tolk"),
    syntax("pack-unpack-8.tolk"),
    syntax("pack-unpack-9.tolk"),
    syntax("strings-tests.tolk"),
    syntax("union-types-tests.tolk"),
];

pub(crate) const TYPE_EXPECTATION_EXCLUSIONS: &[TypeExpectationExclusion] = &[];

// The compiler infers a generic function body only after creating a concrete
// instantiation. The same source member can therefore resolve to different symbols at
// different call sites. The language server currently stores one `InferenceResult` per
// source declaration, so selecting one of those symbols would make go-to-definition and
// references unsound. Supporting these cases requires contextual body inference together
// with multi-target resolutions, rather than a receiver-name fallback.
const CONTEXTUAL_GENERIC_BODY: &str =
    "the compiler resolves this member only after instantiating its generic function body";

pub(crate) const RESOLUTION_EXCLUSIONS: &[ResolutionExclusion] = &[
    unresolved("generics-1.tolk", 111, 48, "first", CONTEXTUAL_GENERIC_BODY),
    unresolved("generics-1.tolk", 112, 46, "first", CONTEXTUAL_GENERIC_BODY),
    unresolved("generics-1.tolk", 113, 64, "push", CONTEXTUAL_GENERIC_BODY),
    unresolved("generics-2.tolk", 123, 14, "value", CONTEXTUAL_GENERIC_BODY),
    unresolved("generics-2.tolk", 128, 17, "value", CONTEXTUAL_GENERIC_BODY),
    unresolved(
        "generics-2.tolk",
        156,
        42,
        "loadInt",
        CONTEXTUAL_GENERIC_BODY,
    ),
    unresolved("struct-tests.tolk", 287, 39, "x", CONTEXTUAL_GENERIC_BODY),
    unresolved("struct-tests.tolk", 287, 49, "y", CONTEXTUAL_GENERIC_BODY),
];

pub(crate) const GLOBAL_RESOLUTION_EXCLUSIONS: &[(&str, &str)] = &[(
    "self",
    "implicit receiver keyword has no source declaration",
)];
