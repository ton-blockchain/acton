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
a non-generated file is an error.

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
`ByCodeHash` normalizes hex or base64 and returns a copy of every matching
contract: neither generation nor lookup picks a winner for an ambiguous hash.

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

## Testing

```sh
go mod tidy
CGO_ENABLED=0 GOPROXY=off GOSUMDB=off go test ./...
```

No test fetches catalog data or needs the indexer or a compiler checkout. Two
fixtures document the ABI shapes they pin:
[boolean enums](codegen/testdata/bool-enum.md) and
[generic serialization hooks](codegen/testdata/review-probe.md).

## License

The shared runtime and generator originated in Toncenter's indexer. The original
MIT license and copyright attribution are preserved in [LICENSE](LICENSE).
