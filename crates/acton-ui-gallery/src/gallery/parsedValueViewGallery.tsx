import {
  ParsedValueView,
  type ContractChipData,
  type ParsedValue,
  type ParsedValueViewProps,
} from "@acton/ui"

import styles from "./parsedValueViewGallery.module.css"
import type {ComponentGallery} from "./types"

const WALLET_ADDRESS = "0:4f3d8fd7bb90f65b46c55cbfc783963f7ec87b8c4e5955666fca73c501cdab10"
const UNKNOWN_ADDRESS = "0:a67149f85f6e6d1ad5d977f98d9f3f7b39324a4c1d879bdd6d1a1fc3f7561f5a"
const FRIENDLY_WALLET_ADDRESS = "EQBPyp_Xu5D2W0bFXL_Hg5Y_fsh7jE5ZVWZvynPFAc2rEHfR"
const CELL_BOC = "b5ee9c7201010101000a00001073656e64"
const EMPTY_CELL_BOC = "b5ee9c72010101010002000000"

const cellValue: ParsedValue = {
  kind: "scalar",
  value: "Cell(b5ee9c720101…1073656e64)",
  rawValue: CELL_BOC,
}

const sliceValue: ParsedValue = {
  kind: "scalar",
  value: "Slice(b5ee9c720101…1073656e64)",
  rawValue: CELL_BOC,
}

const builderValue: ParsedValue = {
  kind: "scalar",
  value: "Builder(b5ee9c720101…1073656e64)",
  rawValue: CELL_BOC,
}

const contracts = new Map<string, ContractChipData>([
  [WALLET_ADDRESS, {displayName: "Wallet V4", letter: "W"}],
])

const nestedValue: ParsedValue = {
  kind: "object",
  typeName: "TransferRequest",
  entries: [
    {key: "queryId", value: {kind: "scalar", value: "1936289396"}},
    {key: "destination", value: {kind: "address", value: WALLET_ADDRESS}},
    {key: "enabled", value: {kind: "boolean", value: true}},
    {
      key: "amounts",
      value: {
        kind: "array",
        items: [
          {kind: "scalar", value: "1500000000", typeName: "coins"},
          {kind: "scalar", value: "250000000"},
        ],
      },
    },
  ],
}

const mapValue: ParsedValue = {
  kind: "map",
  typeName: "dict<int, address>",
  entries: [
    {
      key: {kind: "scalar", value: "1"},
      value: {kind: "address", value: WALLET_ADDRESS},
    },
    {
      key: {kind: "scalar", value: "2"},
      value: {kind: "address", value: UNKNOWN_ADDRESS},
    },
  ],
}

const nestedArrayValue: ParsedValue = {
  kind: "array",
  items: [
    {
      kind: "array",
      items: [
        {kind: "scalar", value: "1"},
        {kind: "scalar", value: "2"},
        {kind: "scalar", value: "3"},
      ],
    },
    {
      kind: "object",
      typeName: "Recipient",
      entries: [
        {key: "address", value: {kind: "address", value: WALLET_ADDRESS}},
        {key: "amount", value: {kind: "scalar", value: "750000000", typeName: "coins"}},
      ],
    },
    {
      kind: "map",
      typeName: "map<int, Cell>",
      entries: [{key: {kind: "scalar", value: "7"}, value: cellValue}],
    },
  ],
}

const nestedMapValue: ParsedValue = {
  kind: "map",
  typeName: "map<address, RecipientState>",
  entries: [
    {
      key: {kind: "address", value: WALLET_ADDRESS},
      value: {
        kind: "object",
        typeName: "RecipientState",
        entries: [
          {key: "active", value: {kind: "boolean", value: true}},
          {
            key: "amounts",
            value: {
              kind: "array",
              items: [
                {kind: "scalar", value: "1000000000", typeName: "coins"},
                {kind: "scalar", value: "250000000", typeName: "coins"},
              ],
            },
          },
          {key: "payload", value: sliceValue},
        ],
      },
    },
    {
      key: {kind: "address", value: UNKNOWN_ADDRESS},
      value: {
        kind: "map",
        typeName: "map<int, bool>",
        entries: [
          {key: {kind: "scalar", value: "0"}, value: {kind: "boolean", value: false}},
          {key: {kind: "scalar", value: "1"}, value: {kind: "boolean", value: true}},
        ],
      },
    },
  ],
}

function formatFriendlyAddress(address: string): string {
  return address === WALLET_ADDRESS ? FRIENDLY_WALLET_ADDRESS : address
}

function valueSample(
  label: string,
  value: ParsedValue,
  props: Omit<ParsedValueViewProps, "value"> = {},
) {
  return (
    <article className={styles.valueSample}>
      <span className={styles.valueLabel}>{label}</span>
      <div className={styles.valueResult}>
        <ParsedValueView value={value} {...props} />
      </div>
    </article>
  )
}

export const parsedValueViewGallery = {
  id: "parsed-value-view",
  title: "ParsedValueView",
  status: "ready",
  summary:
    "ParsedValueView renders a small ABI-independent value tree with consistent scalars, addresses, objects, arrays, maps, and empty states.",
  importStatement: 'import {ParsedValueView} from "@acton/ui"',
  agentSummary:
    "Use ParsedValueView after domain code has decoded ABI or storage data into the exported minimal ParsedValue union. Keep parsing, ABI selection, and address-network policy outside the component.",
  usage: [
    "Convert parser output into the minimal ParsedValue discriminated union before rendering.",
    "Pass fieldName when rendering an isolated scalar that should apply key, subwallet-id, or coin display rules.",
    "Pass ContractChip metadata and formatAddress only when address nodes need richer presentation.",
  ],
  avoid: [
    "Do not pass a full ABI, symbol table, cell, tuple reader, or parser context.",
    "Do not recursively render object and map nodes in callers.",
    "Do not add manual copy state for raw scalar values; rawValue enables the shared inline copy action.",
  ],
  sections: [
    {
      id: "parsed-value-primitives",
      title: "Primitive Values",
      description:
        "Null, void, booleans, decimal values, field-sensitive hexadecimal output, coins, and raw copy are shown independently.",
      content: (
        <div className={styles.valueGrid}>
          {valueSample("null", {kind: "null"})}
          {valueSample("void", {kind: "void"})}
          {valueSample("boolean · true", {kind: "boolean", value: true})}
          {valueSample("boolean · false", {kind: "boolean", value: false})}
          {valueSample("decimal", {kind: "scalar", value: "1936289396"})}
          {valueSample(
            "subwalletId → hex",
            {kind: "scalar", value: "255"},
            {fieldName: "subwalletId"},
          )}
          {valueSample(
            "tonAmount → GRAM",
            {kind: "scalar", value: "1500000000", typeName: "coins"},
            {fieldName: "tonAmount"},
          )}
          {valueSample("raw copy", {
            kind: "scalar",
            value: "0x73656e64",
            rawValue: "b5ee9c7201010101000a00001073656e64",
          })}
        </div>
      ),
    },
    {
      id: "parsed-value-addresses",
      title: "Address Values",
      description:
        "Resolved and unresolved addresses reuse ContractChip while formatting remains caller-owned.",
      content: (
        <div className={styles.valueGrid}>
          {valueSample(
            "known contract",
            {kind: "address", value: WALLET_ADDRESS},
            {contracts, formatAddress: formatFriendlyAddress},
          )}
          {valueSample("unknown contract", {kind: "address", value: UNKNOWN_ADDRESS}, {contracts})}
        </div>
      ),
    },
    {
      id: "parsed-value-cell-like",
      title: "Cell-like and Serialized Values",
      description:
        "Cells, slices, and builders use the scalar boundary: the domain layer supplies a compact preview and raw BOC for copying.",
      content: (
        <div className={styles.valueGrid}>
          {valueSample("Cell", cellValue)}
          {valueSample("Slice", sliceValue)}
          {valueSample("Builder", builderValue)}
          {valueSample("empty Cell", {
            kind: "scalar",
            value: "<empty cell>",
            rawValue: EMPTY_CELL_BOC,
          })}
          {valueSample("empty Slice", {
            kind: "scalar",
            value: "<empty slice>",
            rawValue: EMPTY_CELL_BOC,
          })}
          {valueSample("empty Builder", {
            kind: "scalar",
            value: "<empty builder>",
            rawValue: EMPTY_CELL_BOC,
          })}
          {valueSample("BitString", {
            kind: "scalar",
            value: "x{DEADBEEF}",
            typeName: "bits32",
          })}
        </div>
      ),
    },
    {
      id: "parsed-value-nested",
      title: "Objects and Arrays",
      description:
        "Flat and nested arrays can contain objects, maps, addresses, and serialized cell-like values.",
      content: (
        <div className={styles.structureGrid}>
          {valueSample("object → array", nestedValue, {
            contracts,
            formatAddress: formatFriendlyAddress,
          })}
          {valueSample("array → array / object / map", nestedArrayValue, {
            contracts,
            formatAddress: formatFriendlyAddress,
          })}
        </div>
      ),
    },
    {
      id: "parsed-value-maps",
      title: "Maps",
      description:
        "Map keys and values can be leaf values or nested arrays, objects, and other maps.",
      content: (
        <div className={styles.structureGrid}>
          {valueSample("map<int, address>", mapValue, {contracts})}
          {valueSample("map → object / array / map", nestedMapValue, {
            contracts,
            formatAddress: formatFriendlyAddress,
          })}
        </div>
      ),
    },
    {
      id: "parsed-value-empty-states",
      title: "Empty States",
      description: "Empty arrays, objects, and maps remain explicit without decorative containers.",
      content: (
        <div className={styles.valueGrid}>
          {valueSample("empty array", {kind: "array", items: []})}
          {valueSample("empty object", {kind: "object", entries: []})}
          {valueSample("empty map", {kind: "map", typeName: "dict", entries: []})}
        </div>
      ),
    },
  ],
} satisfies ComponentGallery
