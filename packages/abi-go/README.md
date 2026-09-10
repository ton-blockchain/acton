# Acton Native Go ABI Bindings

`github.com/ton-blockchain/acton/packages/abi-go` is a standalone **Go 1.26.3**
module. Its root package, `acton`, is a pure Go facade and shared codec library
using **tonutils-go v1.15.5**. The `codegen` subpackage is a reusable build-time
compiler ABI validator and Go generator.
Generated code calls native codec constructors once at initialization. It does
not interpret a type table or parse compiler ABI JSON while handling requests.
The embedded `Contract.ABI` is only for metadata export.

This module contains the shared runtime, generator, CLI and small self-contained
test fixtures. Applications own their catalog inputs and generated bindings.

## Generation

Run from `packages/abi-go` in the Acton checkout:

```sh
go run ./cmd/tolk-abi-to-go --catalog FILE --output-dir DIR --package catalog
go run ./cmd/tolk-abi-to-go --catalog FILE --output-dir DIR --package catalog --snapshot
go run ./cmd/tolk-abi-to-go --abi FILE --output-dir DIR --package catalog
go run ./cmd/tolk-abi-to-go --catalog FILE --output-dir DIR --package catalog --check
CGO_ENABLED=0 go test ./...
CGO_ENABLED=0 go vet ./...
```

The CLI's canonical package path is
`github.com/ton-blockchain/acton/packages/abi-go/cmd/tolk-abi-to-go`.

Exactly one of `--catalog` and `--abi` is required. `--output-dir` is required;
`--package` defaults to `catalog`. A single ABI uses `contract_name` as its ID and
display name, with no hashes or addresses. The catalog envelope is:

```json
{
  "schemaVersion": 1,
  "contracts": [{
    "id": "example",
    "displayName": "Example",
    "hashes": [],
    "knownAddresses": [],
    "links": [{"kind": "source", "title": "Source", "url": "https://example.org"}],
    "compilerAbi": {}
  }]
}
```

Replace `compilerAbi` with a complete raw compiler ABI, including all four type
tables, storage, getter and message tables. Missing fields, null indexes, bad
references, inconsistent monomorphizations, duplicate declarations/IDs and
invalid numeric widths fail generation. Unknown type kinds and unsupported
layouts produce **per-root diagnostics**, not an unusable entire contract.
The CLI prints diagnostics to stderr; unsupported roots do not change the exit
status. Their `Unsupported` metadata is nonempty and their functions are nil.

Catalog `links` and `knownAddresses` may be omitted, matching Acton's defaults.
When supplied, they must be arrays of valid entries, not null. Both generic
declaration names and fully qualified instantiated names are accepted in the
compiler's monomorphization tables; indexes and instantiated names must agree.

`--snapshot` (or `Options.Snapshot`) additionally writes the exact catalog input
to `catalog.json`. Snapshot replacement requires its current digest to match the
input digest recorded by the previously generated registry. This prevents an
unrelated or locally modified JSON file from being overwritten. The option is
catalog-only and works with `--check`; later regeneration from that snapshot
does not need `--snapshot`.

`--check` never writes. It fails on missing, modified or stale generated files.
Normal generation atomically replaces each changed file, and removes stale files
carrying this generator's exact header. Unrelated files are preserved; overwriting
a non-generated file is an error. The operation is atomic per file, not across
the entire directory. Do not run competing generators in the same output directory.

Programmatic use imports
`github.com/ton-blockchain/acton/packages/abi-go/codegen`:

```go
out, err := codegen.Generate(data, codegen.Options{Package: "catalog"})
if err != nil { return err }
// out.Files contains formatted source; out.Diagnostics contains root failures.
return out.Write(outputDirectory, false)
```

The output contains one file per contract plus `registry_gen.go`, exporting:

```go
var Contracts []*acton.Contract
const Revision string // SHA-256 of the exact input bytes
func ByID(string) *acton.Contract
func ByCodeHash(string) []*acton.Contract
```

Contracts are sorted by ID; hashes are normalized, deduplicated and sorted.
`ByCodeHash` normalizes hex or base64 and preserves multiple matching contracts.
Neither generation nor lookup chooses an arbitrary winner for ambiguous hashes.
Lookup maps are initialized once; requests do not scan `Contracts`. Hash lookup
returns a copy of the result slice. Pointed-to metadata remains shared and
immutable by convention. Codec closures have no request-shared mutable state.

## Facade

The public package retains the name `acton`:

```go
import acton "github.com/ton-blockchain/acton/packages/abi-go"
```

- `Contract`: ID, display name, hashes, known addresses, links, embedded ABI,
  getters, runtime/deployment storage bindings and four message directions.
- `Binding.Encode(any)` and `Binding.Decode(*cell.Cell)`: native cell codecs.
- `GetMethod.EncodeArgs(map[string]any)` and `DecodeResult([]StackValue)`:
  declared-order TVM stack conversion with full consumption.
- `DecodeStorage(contract, base64BOC)`: try runtime layout, then deployment
  layout; each attempt strictly consumes its entire cell.
- `DecodeMessage(contract, direction, base64BOC)`: strictly decode candidates,
  rejecting zero matches and multiple matches.
- `NormalizeCodeHash`: canonical lowercase 32-byte hex from hex/base64.
- `DecodeBOC`: bounded base64 BOC input with one ordinary root. Validated opaque
  descendants are permitted at raw-cell boundaries.
- `DecodeOpaqueBOC`: bounded BOC parser accepting ordinary cells and validated
  library references, pruned branches, Merkle proofs and Merkle updates.

Message direction keys are exactly `incoming_messages`, `incoming_external`,
`outgoing_messages`, `emitted_events`. Functions are excluded from metadata JSON.

## Value Format

- All Tolk integers and integer-backed enum values decode to **decimal strings**,
  including small integers, coins and signed variable integers. Encoding also accepts Go
  integers, `json.Number`, and `big.Int`/`*big.Int`, without mutating the input.
  Floating-point Tolk integers are rejected rather than rounded.
  Getter encoding and decoding enforce only the signed 257-bit TVM range;
  `uint8`, for example, can carry 256 on the stack. Declared integer widths and
  variable-integer bounds remain enforced for cell serialization.
- Booleans are JSON booleans. TVM true is encoded as `-1`; decoding treats any
  valid nonzero TVM integer as true.
- Boolean-backed enum ABI cell values are JSON booleans (`false`/`true`), encoded
  as one bit. Their generated cell types and constants are named Go booleans.
  Direct enum getter values are **unsupported**, including through aliases,
  nullable values, arrays and structs: the ABI's boolean member strings do not
  specify an integer enum stack mapping. Such methods retain `Unsupported`
  metadata with nil callbacks and `any` for the unresolved stack type.
  `Cell<Enum>` getter payloads use the verified boolean cell representation and
  remain supported. Ordinary Tolk `bool` getter behavior above is unaffected.
- Structs are objects with original field names. No discriminator is added to
  plain structs. Native Go structs with matching JSON tags can also be encoded.
- Tensors and shaped tuples are JSON arrays. Their TVM layouts differ:
  tensors flatten, shaped tuples box wide elements. Arrays and Lisp lists
  likewise box elements whose stack width is not one.
- Standard addresses are canonical raw `workchain:hex` strings. Optional
  addresses are null or strings, with **no extra Maybe bit** in cells.
  External addresses use `{"bits": 5, "hex": "a8"}`. `addressAny` additionally
  accepts null. Anycast and variable internal addresses are explicitly rejected.
- Bits use `acton.Bits`, JSON `{"bits": 5, "hex": "a8"}`: MSB-first bytes,
  exact bit count, zero padding in the low bits of the final byte. The small
  `bits` count accepts JSON numeric values as well as exact Go integers.
- Raw cells, getter slices/builders and `RemainingBitsAndRefs` are base64 BOCs.
  Encoding also accepts `*cell.Cell`. Remaining consumes all bits **and refs**;
  a plain `slice` is not silently treated as Remaining for cell decoding.
  Raw cell references and cell-valued stack items preserve validated opaque
  cells, including native pointers inside nullable values. Typed slices and
  struct layouts never interpret an exotic cell as ordinary payload data.
- `Cell<T>` exposes the **decoded T payload directly**, not a `{ref: ...}`
  wrapper, and requires complete consumption of the referenced cell.
- Dictionaries are sorted binary-key-order `[]acton.MapEntry`, JSON
  `[{"key": ..., "value": ...}]`. Keys retain their types. Duplicate encoded
  keys are rejected. Values use their declared inline cell layout.
  Raw `slice` values and hook-free aliases of `slice` are supported specifically
  at a dictionary leaf boundary, where they consume all remaining bits and refs.
- Unions always use `acton.UnionValue`, JSON `{"$": "RenderedType", "value": ...}`,
  including struct variants. A null variant is JSON null. A void variant is
  `{"$":"void","value":null}`. Labels are rendered compiler type names.
- Strings are UTF-8 snake strings. Cell serialization is a ref to the snake;
  getter serialization is the snake cell itself.

`StackValue` has JSON fields `type` and `value`. Supported types are `int`,
`null`, `cell`, `slice`, `builder`, `tuple`. Cell-like values are base64 BOCs;
tuple values are arrays of `StackValue`. Integers are decimal strings. Stack
arrays are in declared order, not reversed. Wide nullable/union tags and padding
are checked against compiler metadata, never inferred from client field types.

## Generated Types

Contract namespaces are `C<SanitizedID>_<first16hexOfSHA256ID>`. Hash collisions
are checked before writing. Struct, enum and alias names append their sanitized
declaration name and `T<unique_type_index>`. Generic instantiations therefore
cannot collide with each other or with the generic declaration.

Struct fields append `F<field_index>` and preserve their exact original JSON tag.
Integer-backed enums are string-backed named types with decimal-string constants;
boolean-backed cell enums use named bool types with boolean constants. Enum
constants are suffixed `M<member_index>`. Aliases are Go aliases where representable.
Compound types use
arrays, pointers, `acton.Bits`, `acton.MapEntry` and `acton.UnionValue`; otherwise
they use `any`. Pointer struct references support recursive cell payloads.

Every struct also has a `Stack` version using **declared** field types. Its cell
version honors `client_ty_idx`. Getter argument/result definitions are named
`<Namespace><Method>G<method_index>Args` and `...Result`; they use stack types,
except that typed-cell payloads use their cell types. These definitions describe
the JSON-safe values. The public callable bindings deliberately retain the
uniform map/`any` facade; decoding returns JSON-safe maps rather than pointers
to generated Go structs.

## Support And Limits

Native codecs cover fixed/signed/variable integers, coins, booleans, addresses,
bits, strings, raw/typed refs, nullable values, tensors/shaped tuples, arrays,
Lisp lists, fixed-bit-key dictionaries, structs, enums, aliases and unions.
Array writing uses compiler maximum-size chunking, with continuation refs before
element refs. Reading accepts any valid compiler chunk occupancy. Lisp lists
use the stdlib's reversed snake representation, also storing the tail ref first.
Enum cell decoding validates membership, matching the compiler.

Custom pack/unpack hooks have only flags in the ABI, not executable bodies.
Flags on both declarations and concrete struct/alias instantiations are checked.
Cell roots depending on them are unsupported; plain getter structs can still work
because hooks do not alter their declared stack layout. `int` is getter-only.
`slice` and `builder` have getter codecs but no inferred cell decoder. Callable,
unknown, unresolved generic types, ambiguous union prefixes, missing wide stack
metadata and non-fixed dictionary keys are explicitly unsupported.

Capability analysis rejects a remainder followed by another consuming field,
including remainders reached through aliases, structs, tuples, nullables or union
branches. Reference and dictionary boundaries contain the remainder effect;
zero-size trailing fields are allowed. The remainder codec itself is unchanged.

Method defaults are compiled to Go factories. Exact integers, booleans, strings,
addresses, nulls, bits slices, tensors/shaped tuples, objects, and supported
representation-preserving casts are handled. Unsupported method defaults disable
that method explicitly. Unsupported struct-field defaults error when that field
is omitted; callers can provide the field explicitly. Runtime encoding still
checks ranges and types of supported defaults.

Boolean-backed enum cell fields accept boolean defaults (including aliases and
representation-preserving boolean casts). Integer-to-boolean enum default
conversions are explicitly rejected; no mapping from `0`, `1` or `-1` is guessed.
Boolean-enum getter defaults do not enable an otherwise unsupported stack root.

Each call enforces 128 levels of codec/cell/stack nesting, 16,384 traversed values,
4,096 BOC cells and a 1 MiB data budget. Arrays have the TVM limit of 255 elements.
Bounded dictionary traversal handles shared DAGs without exponential expansion.
BOC headers and counts are validated **before** constructing dependency cells.
Complete, forward-reference, single-root BOCs with standard magic are accepted;
CRC, index tables and validated stored hashes/levels are supported. Opaque parsing
checks exotic descriptors, level masks, embedded proof hashes/depths and pruned
virtual depths. Absent cells, unknown exotic types and multi-root BOCs are
rejected. These limits can reject otherwise valid large TON values.
`DecodeBOC` additionally requires an ordinary root; typed decoding rejects any
attempt to interpret exotic payloads. `DecodeOpaqueBOC` does not resolve a library
or authenticate a proof against a trusted chain root. A library-reference cell's
`Hash()` is its representation hash; its embedded library ID is the separate
256-bit value after the 8-bit library tag.
Malformed inputs return errors; dependency panic paths are contained at public
codec boundaries. No `Must` operations are used by this implementation.

## References And Tests

The schema follows Acton's `crates/tolk-source-map/src/abi.rs` and
`types_kernel.rs`. Cell layouts follow the checked-in compiler's
`tolk/pack-unpack-serializers.cpp`, with Lisp lists following
`crypto/smartcont/tolk-stdlib/lisp-lists.tolk`. The installed
`@ton/tolk-abi-to-typescript/dist` is a secondary stack/runtime reference.
The compiler is authoritative where that runtime differs, notably Lisp ref
ordering, enum validation, and the getter string cell representation.
TON's [Start Here](https://docs.ton.org/start-here) provides the cell/BOC and
getter terminology; no network or external compiler is needed by this code.

Tests include fixed bit vectors, signed varint boundaries, `int257` minimum and
input immutability, optional addresses, explicit/implicit unions, strict typed
refs, compiler array chunking, inline dictionaries, malformed labels, getter
boxing/tags/defaults, schema failures, deterministic output, check-mode behavior,
and fuzz targets for BOC/native decode. A generated two-contract package is
compiled and executed in a temporary module with CGO disabled, using a local
replacement for this module and network module lookups disabled. After populating
the dependency cache, the entire suite can run offline:

```sh
go mod tidy
CGO_ENABLED=0 GOPROXY=off GOSUMDB=off go test ./...
CGO_ENABLED=0 GOPROXY=off GOSUMDB=off go vet ./...
```

No test fetches catalog data or requires the indexer or a compiler checkout.

The [boolean-enum regression fixture](codegen/testdata/bool-enum.md) documents the
synthetic ABI used by Acton's Rust unpacker tests, independent one-bit cell
goldens, TypeScript runtime observations and the explicit stack limitation.

## License

The shared runtime and generator originated in Toncenter's indexer. The original
MIT license and copyright attribution are preserved in [LICENSE](LICENSE).
