# Boolean-backed enum regression

`bool-enum.abi.json` is a **synthetic ABI**, extending the `Toggle` enum built by
Acton's `crates/tolk-source-map/src/dynamic_unpack.rs` test
`decodes_bool_encoded_enum_to_raw_value_like_ts_dynamic_unpack` (line 1539 at
Acton commit `4b21721a1`). It is not claimed to be output from a Tolk compiler.

The fixture retains that test's `encoded_as_ty_idx` pointing to `bool` and its
`Off = "false"` / `On = "true"` member strings. Additional declarations exercise
aliases, field defaults, nullable/array/struct stack roots, and typed-cell getters.
A second, unrelated contract is added by the Go test so the regression proves
that this ABI no longer aborts an entire catalog. The generated package is
compiled and executed, including typed constants, structs, and getter types.

## Independently checked cell values

These are ordinary one-bit cells built with `@ton/core` and decoded through
`@ton/tolk-abi-to-typescript` 0.5.0 `dynamicUnpack` (both direct and alias-backed
boolean enum encodings):

| JSON value | Bits (Fift top-up notation) | Base64 BoC |
| --- | --- | --- |
| `false` | `4_` | `te6cckEBAQEAAwAAAUD20kA0` |
| `true` | `C_` | `te6cckEBAQEAAwAAAcCO6ba2` |

Go encoding accepts JSON booleans and generated named boolean constants. It
rejects strings such as `"true"`, numeric values, and null. Decoding checks enum
membership; an enum containing only `true` rejects a false cell. As with integer
enums, encoding uses the declared cell representation; membership is checked on
decode. Struct defaults `false` and an aliased `true` produce exactly bits `01`.
`Cell<Toggle>` getter arguments/results use one `cell` stack item; the public Go
value is the decoded boolean payload.

## Why direct enum stack roots are explicitly unsupported

[TON Docs: Enums](https://docs.ton.org/tolk/types/enums) specifies integer enum
members, integer TVM values, and `(u)intN` serialization. The reviewed compiler's
`tolk/pipe-check-serialized-fields.cpp:129-145` rejects a `bool` enum serialization
type; `tolk/constant-evaluator.cpp:698-729` rejects non-integer member initializers.
Consequently there is no compiler-produced boolean enum fixture or TVM smoke
result establishing a mapping for these synthetic boolean member strings.

The installed TypeScript runtime confirms the distinction:

- `dynamic-serialization.js:371-376` unpacks an enum through its encoded type,
  giving the boolean cell result above.
- `dynamic-serialization.js:227-234` first checks a numeric enum value on pack,
  so it cannot pack this fixture using boolean inputs.
- `dynamic-get-methods.js:287-290,395` constructs/parses enum stack values as raw
  integers. A mock-provider probe preserves `0`, `-1`, `1`, and `42`; boolean
  inputs are rejected. This is a runtime representation probe, not a TVM test.
- Ordinary Tolk `bool` has its own `true -> -1` mapping. Applying it to this enum
  would invent semantics unsupported by the compiler and the reference runtime.

Go therefore preserves explicit per-method `Unsupported` metadata and nil
callbacks for direct boolean enum stack roots, including nested forms and
defaults. Their unresolved generated stack type is `any`; their cell types are
named booleans. Typed-cell getters and unrelated bool/integer getters still work.
Integer enum member strings and their existing stack codecs are unchanged.

Malformed boolean member strings have contextual generation errors naming the
enum and member. Only the exact ABI spellings `"false"` and `"true"` are accepted.
Integer enum defaults are not converted to booleans: integer defaults on these
cell-only enums produce explicit rejected-default errors.

## Reproduction

The Go regression needs no Node, compiler, network, or external checkout after
dependencies are cached:

```sh
CGO_ENABLED=0 GOPROXY=off GOSUMDB=off GOWORK=off go test ./...
```

Optional reference probe, from `packages/abi-go` with Acton's UI dependencies
already installed:

```sh
node codegen/testdata/bool-enum-reference.cjs ../explorer-core/node_modules
```
