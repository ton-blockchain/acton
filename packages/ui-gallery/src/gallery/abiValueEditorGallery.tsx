import {
  AbiValueEditor,
  SAMPLE_ADDRESS,
  SAMPLE_EXTERNAL_ADDRESS,
  TON_ADDR_NONE,
  abiValueToFormValue,
  createAbiSymbols,
  decodeAbiValueFromBoc,
  encodeAbiValueToBoc,
  stringifyAbiJson,
  type ContractABI,
} from "@acton/transaction-ui/abi"
import {Button, RawDataBlock, shortenMiddle} from "@acton/ui"
import {useMemo, useState} from "react"

import styles from "./abiValueEditorGallery.module.css"
import type {ComponentGallery} from "./types"

const EMPTY_CELL_BOC = "b5ee9c72010101010002000000"

const typeIndex = {
  slice: 2,
  cell: 3,
  builder: 4,
  scalarFields: 14,
  optionalInt: 13,
  nestedStruct: 16,
  array: 17,
  union: 24,
  collectionFields: 25,
  cellFields: 26,
  mapFields: 27,
  arrayOfStructs: 28,
  structsByBatch: 29,
  nestedCollections: 30,
  complexCollectionFields: 31,
  optionalTon: 32,
  addressFields: 35,
} as const

const matrixAbi = {
  contract_name: "AbiValueEditorGallery",
  compiler_name: "tolk",
  compiler_version: "gallery",
  declarations: [
    {
      kind: "struct",
      name: "ScalarFields",
      ty_idx: typeIndex.scalarFields,
      fields: [
        {name: "count", ty_idx: 8},
        {name: "enabled", ty_idx: 5},
        {name: "recipient", ty_idx: 7},
        {name: "tonAmount", ty_idx: 6},
        {name: "maybeAddress", ty_idx: 12},
        {name: "maybeCount", ty_idx: typeIndex.optionalInt},
        {name: "maybeTon", ty_idx: typeIndex.optionalTon},
      ],
    },
    {
      kind: "alias",
      name: "CounterAlias",
      ty_idx: 15,
      target_ty_idx: 8,
    },
    {
      kind: "struct",
      name: "NestedStruct",
      ty_idx: typeIndex.nestedStruct,
      fields: [
        {name: "summary", ty_idx: typeIndex.scalarFields},
        {name: "counterAlias", ty_idx: 15},
      ],
    },
    {
      kind: "struct",
      name: "CollectionFields",
      ty_idx: typeIndex.collectionFields,
      fields: [
        {name: "array", ty_idx: typeIndex.array},
        {name: "list", ty_idx: 18},
        {name: "tuple", ty_idx: 19},
      ],
    },
    {
      kind: "struct",
      name: "CellFields",
      ty_idx: typeIndex.cellFields,
      fields: [
        {name: "cell", ty_idx: typeIndex.cell},
        {name: "slice", ty_idx: typeIndex.slice},
        {name: "builder", ty_idx: typeIndex.builder},
        {name: "bits8", ty_idx: 22},
      ],
    },
    {
      kind: "struct",
      name: "MapFields",
      ty_idx: typeIndex.mapFields,
      fields: [
        {name: "byId", ty_idx: 20},
        {name: "byOwner", ty_idx: 21},
      ],
    },
    {
      kind: "struct",
      name: "ComplexCollectionFields",
      ty_idx: typeIndex.complexCollectionFields,
      fields: [
        {name: "records", ty_idx: typeIndex.arrayOfStructs},
        {name: "recordsByBatch", ty_idx: typeIndex.structsByBatch},
        {name: "nestedBatches", ty_idx: typeIndex.nestedCollections},
      ],
    },
    {
      kind: "struct",
      name: "AddressFields",
      ty_idx: typeIndex.addressFields,
      fields: [
        {name: "internal", ty_idx: 7},
        {name: "external", ty_idx: 33},
        {name: "any", ty_idx: 34},
      ],
    },
  ],
  unique_types: [
    {kind: "void"},
    {kind: "int"},
    {kind: "slice"},
    {kind: "cell"},
    {kind: "builder"},
    {kind: "bool"},
    {kind: "coins"},
    {kind: "address"},
    {kind: "intN", n: 32},
    {kind: "uintN", n: 32},
    {kind: "intN", n: 64},
    {kind: "uintN", n: 64},
    {kind: "addressOpt"},
    {kind: "nullable", inner_ty_idx: 8},
    {kind: "StructRef", struct_name: "ScalarFields"},
    {kind: "AliasRef", alias_name: "CounterAlias"},
    {kind: "StructRef", struct_name: "NestedStruct"},
    {kind: "arrayOf", inner_ty_idx: 8},
    {kind: "lispListOf", inner_ty_idx: 5},
    {kind: "shapedTuple", items_ty_idx: [8, 5, 7]},
    {kind: "mapKV", key_ty_idx: 9, value_ty_idx: 6},
    {kind: "mapKV", key_ty_idx: 7, value_ty_idx: 3},
    {kind: "bitsN", n: 8},
    {kind: "nullLiteral"},
    {
      kind: "union",
      variants: [
        {variant_ty_idx: 23, prefix_num: 0, prefix_len: 1, is_prefix_implicit: true},
        {
          variant_ty_idx: typeIndex.scalarFields,
          prefix_num: 1,
          prefix_len: 1,
          is_prefix_implicit: true,
        },
      ],
    },
    {kind: "StructRef", struct_name: "CollectionFields"},
    {kind: "StructRef", struct_name: "CellFields"},
    {kind: "StructRef", struct_name: "MapFields"},
    {kind: "arrayOf", inner_ty_idx: typeIndex.scalarFields},
    {kind: "mapKV", key_ty_idx: 9, value_ty_idx: typeIndex.arrayOfStructs},
    {kind: "arrayOf", inner_ty_idx: typeIndex.structsByBatch},
    {kind: "StructRef", struct_name: "ComplexCollectionFields"},
    {kind: "nullable", inner_ty_idx: 6},
    {kind: "addressExt"},
    {kind: "addressAny"},
    {kind: "StructRef", struct_name: "AddressFields"},
  ],
  struct_instantiations: [],
  alias_instantiations: [],
  storage: {storage_ty_idx: typeIndex.nestedStruct},
  incoming_messages: [],
  incoming_external: [],
  outgoing_messages: [],
  emitted_events: [],
  get_methods: [],
  thrown_errors: [],
} satisfies ContractABI

const matrixSymbols = createAbiSymbols(matrixAbi)
const treasuryAddress = "EQAREREREREREREREREREREREREREREREREREREREREREeYT"

const addressSuggestions = [
  {address: SAMPLE_ADDRESS, label: `Zero account · ${shortAddress(SAMPLE_ADDRESS)}`},
  {address: treasuryAddress, label: `Treasury · ${shortAddress(treasuryAddress)}`},
] as const

function shortAddress(address: string): string {
  return shortenMiddle(address, {start: 6, end: 6})
}

const scalarValue = {
  count: "7",
  enabled: true,
  recipient: SAMPLE_ADDRESS,
  tonAmount: "1250000000",
  maybeAddress: null,
  maybeCount: "42",
  maybeTon: "2500000000",
}

const nestedValue = {
  summary: scalarValue,
  counterAlias: "99",
}

const collectionValue = {
  array: ["1", "2", "3"],
  list: [true, false, true],
  tuple: ["11", true, SAMPLE_ADDRESS],
}

const mapValue = {
  byId: {"1": "1000000000", "7": "250000000"},
  byOwner: {[SAMPLE_ADDRESS]: EMPTY_CELL_BOC},
}

const alternateScalarValue = {
  ...scalarValue,
  count: "12",
  enabled: false,
  tonAmount: "750000000",
  maybeAddress: SAMPLE_ADDRESS,
  maybeCount: null,
}

const complexCollectionValue = {
  records: [scalarValue, alternateScalarValue],
  recordsByBatch: {
    "1": [scalarValue],
    "7": [alternateScalarValue],
  },
  nestedBatches: [
    {
      "42": [alternateScalarValue, scalarValue],
    },
  ],
}

const cellValue = {
  cell: EMPTY_CELL_BOC,
  slice: EMPTY_CELL_BOC,
  builder: EMPTY_CELL_BOC,
  bits8: EMPTY_CELL_BOC,
}

function EditorSample({
  title,
  tyIdx,
  initialValue,
  disabled,
  invalid,
  label,
}: {
  readonly title: string
  readonly tyIdx: number
  readonly initialValue: unknown
  readonly disabled?: boolean
  readonly invalid?: boolean
  readonly label?: string
}) {
  const [value, setValue] = useState(initialValue)

  return (
    <article className={styles.scenario}>
      <div className={styles.scenarioHeader}>
        <h4>{title}</h4>
        <code>ty#{tyIdx}</code>
      </div>
      <AbiValueEditor
        symbols={matrixSymbols}
        tyIdx={tyIdx}
        value={value}
        onChange={setValue}
        disabled={disabled}
        invalid={invalid}
        label={label}
        addressSuggestions={addressSuggestions}
      />
    </article>
  )
}

function ScalarSamples() {
  return (
    <div className={styles.matrix}>
      <EditorSample
        title="Integer · boolean · address · GRAM"
        tyIdx={typeIndex.scalarFields}
        initialValue={scalarValue}
      />
      <EditorSample
        title="Optional integer · null"
        tyIdx={typeIndex.optionalInt}
        initialValue={null}
      />
      <EditorSample
        title="Optional integer · loaded"
        tyIdx={typeIndex.optionalInt}
        initialValue="64"
      />
      <EditorSample
        title="Optional TON · loaded"
        tyIdx={typeIndex.optionalTon}
        initialValue="1500000000"
        label="tonAmount"
      />
      <EditorSample
        title="Internal · external · any address"
        tyIdx={typeIndex.addressFields}
        initialValue={{
          internal: SAMPLE_ADDRESS,
          external: SAMPLE_EXTERNAL_ADDRESS,
          any: TON_ADDR_NONE,
        }}
      />
    </div>
  )
}

function StructuredSamples() {
  return (
    <div className={styles.matrix}>
      <EditorSample
        title="Struct · alias · nested struct"
        tyIdx={typeIndex.nestedStruct}
        initialValue={nestedValue}
      />
      <EditorSample
        title="Union · selected struct"
        tyIdx={typeIndex.union}
        initialValue={{$: "ScalarFields", ...scalarValue}}
      />
    </div>
  )
}

function CollectionSamples() {
  return (
    <div className={styles.matrix}>
      <EditorSample
        title="Array · list · shaped tuple"
        tyIdx={typeIndex.collectionFields}
        initialValue={collectionValue}
      />
      <EditorSample
        title="Map · uint and address keys"
        tyIdx={typeIndex.mapFields}
        initialValue={mapValue}
      />
      <EditorSample
        title="Nested collections · complex values"
        tyIdx={typeIndex.complexCollectionFields}
        initialValue={complexCollectionValue}
      />
      <EditorSample title="Empty array" tyIdx={typeIndex.array} initialValue={[]} />
      <EditorSample title="Loaded array" tyIdx={typeIndex.array} initialValue={["3", "5", "8"]} />
    </div>
  )
}

function TonSamples() {
  return (
    <div className={styles.matrix}>
      <EditorSample
        title="Cell · Slice · Builder · bitsN"
        tyIdx={typeIndex.cellFields}
        initialValue={cellValue}
      />
    </div>
  )
}

function StateSamples() {
  return (
    <div className={styles.matrix}>
      <EditorSample
        title="Disabled"
        tyIdx={typeIndex.scalarFields}
        initialValue={scalarValue}
        disabled
      />
      <EditorSample
        title="Invalid"
        tyIdx={typeIndex.scalarFields}
        initialValue={{...scalarValue, recipient: "not-an-address"}}
        invalid
      />
    </div>
  )
}

function RoundTripSample() {
  const [value, setValue] = useState<unknown>(nestedValue)
  const result = useMemo(() => {
    try {
      const boc = encodeAbiValueToBoc(matrixAbi, typeIndex.nestedStruct, value)
      const decoded = decodeAbiValueFromBoc(matrixAbi, typeIndex.nestedStruct, boc)
      return {boc, decoded: stringifyAbiJson(abiValueToFormValue(decoded))}
    } catch (error) {
      return {error: error instanceof Error ? error.message : "Round-trip failed"}
    }
  }, [value])

  return (
    <div className={styles.roundTrip}>
      <article className={styles.scenario}>
        <div className={styles.scenarioHeader}>
          <h4>Editable ABI value</h4>
          <code>value → Cell → value</code>
        </div>
        <AbiValueEditor
          symbols={matrixSymbols}
          tyIdx={typeIndex.nestedStruct}
          value={value}
          onChange={setValue}
          invalid={Boolean(result.error)}
          addressSuggestions={addressSuggestions}
        />
      </article>
      <div className={styles.roundTripOutput}>
        {result.error ? (
          <p className={styles.error} role="alert">
            {result.error}
          </p>
        ) : (
          <>
            <RawDataBlock
              title="Serialized BoC"
              value={result.boc ?? ""}
              copyLabel="serialized BoC"
              wrap
              collapsible
              defaultExpanded
              maxHeight={180}
            />
            <RawDataBlock
              title="Decoded form value"
              value={result.decoded ?? ""}
              copyLabel="decoded form value"
              wrap
              collapsible
              defaultExpanded
              maxHeight={260}
            />
          </>
        )}
      </div>
    </div>
  )
}

function createDecodedInitialValue(preset: "primary" | "alternate") {
  if (preset === "alternate") {
    return {
      summary: {
        count: 256n,
        enabled: false,
        recipient: SAMPLE_ADDRESS,
        tonAmount: 2_750_000_000n,
        maybeAddress: SAMPLE_ADDRESS,
        maybeCount: null,
        maybeTon: 500_000_000n,
      },
      counterAlias: 901n,
    }
  }

  return {
    summary: {
      count: 128n,
      enabled: true,
      recipient: SAMPLE_ADDRESS,
      tonAmount: 1_500_000_000n,
      maybeAddress: null,
      maybeCount: 64n,
      maybeTon: null,
    },
    counterAlias: 900n,
  }
}

function InitialValueSample() {
  const [initialValue, setInitialValue] = useState<unknown>(() =>
    createDecodedInitialValue("primary"),
  )
  const [value, setValue] = useState<unknown>({})

  return (
    <div className={styles.roundTrip}>
      <article className={styles.scenario}>
        <div className={styles.scenarioHeader}>
          <h4>Decoded value → editable form</h4>
          <div className={styles.initialValueActions}>
            <Button
              size="sm"
              variant="secondary"
              onClick={() => setInitialValue(createDecodedInitialValue("primary"))}
            >
              Load preset A
            </Button>
            <Button
              size="sm"
              variant="secondary"
              onClick={() => setInitialValue(createDecodedInitialValue("alternate"))}
            >
              Load preset B
            </Button>
          </div>
        </div>
        <AbiValueEditor
          symbols={matrixSymbols}
          tyIdx={typeIndex.nestedStruct}
          value={value}
          initialValue={initialValue}
          onChange={setValue}
          addressSuggestions={addressSuggestions}
        />
      </article>
      <RawDataBlock
        title="Controlled form value"
        value={stringifyAbiJson(value)}
        copyLabel="controlled form value"
        wrap
        collapsible
        defaultExpanded
        maxHeight={320}
      />
    </div>
  )
}

export const abiValueEditorGallery = {
  id: "abi-value-editor",
  title: "ABI Value Editor",
  status: "ready",
  summary:
    "Schema-driven editor for Tolk ABI values, including TON addresses, GRAM amounts, dictionaries, and Cell-like values.",
  importStatement: 'import { AbiValueEditor } from "@acton/transaction-ui/abi"',
  agentSummary:
    "Keep ABI-aware editing and serialization in transaction-ui. Pass a SymTable, type index, controlled form value, optional decoded initial value, and validation state; keep ABI registry loading and network state in the consuming app.",
  usage: [
    "Use for ABI-described message bodies, storage, and other TON tooling inputs.",
    "Keep the value controlled so form and JSON modes can share one source of truth.",
    "Pass initialValue when a decoded runtime ABI value must hydrate the controlled form after the editor mounts.",
    "Use transaction-ui serialization helpers for Cell/BoC round-trips instead of rebuilding type switches in a page.",
  ],
  avoid: [
    "Do not use for neutral parsed output; use ParsedValueView from @acton/ui.",
    "Do not fetch ABIs or perform network requests inside the editor.",
    "Do not treat the gallery as serialization verification; cover wire behavior with automated tests.",
  ],
  sections: [
    {
      id: "abi-scalars",
      title: "Scalars and Optional Values",
      description: "Integer, boolean, address, GRAM, optional, null, and loaded values.",
      content: <ScalarSamples />,
    },
    {
      id: "abi-structures",
      title: "Structures and Unions",
      description: "Struct, alias, nested struct, and explicit union selection.",
      content: <StructuredSamples />,
    },
    {
      id: "abi-collections",
      title: "Collections",
      description:
        "Array, list, shaped tuple, map with multiple key/value shapes, empty, and populated states.",
      content: <CollectionSamples />,
    },
    {
      id: "abi-ton-values",
      title: "TON Values",
      description: "Cell, Slice, Builder, and bitsN BoCs.",
      content: <TonSamples />,
    },
    {
      id: "abi-states",
      title: "Interaction States",
      description: "Disabled controls and an externally invalid editor state.",
      content: <StateSamples />,
    },
    {
      id: "abi-initial-value",
      title: "Decoded Initial Value",
      description:
        "Apply a decoded runtime value after mount, keep editing it, or load another decoded value.",
      content: <InitialValueSample />,
    },
    {
      id: "abi-round-trip",
      title: "Visual Round-trip",
      description:
        "The gallery exposes the visual flow; automated tests verify serialization behavior.",
      content: <RoundTripSample />,
    },
  ],
} satisfies ComponentGallery
