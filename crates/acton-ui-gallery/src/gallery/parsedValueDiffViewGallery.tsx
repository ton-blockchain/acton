import {
  buildStorageDiff,
  type ContractChipData,
  type ParsedStorageValue,
  type ParsedValueDiff,
  ParsedValueDiffView,
} from "@acton/ui"

import styles from "./parsedValueDiffViewGallery.module.css"
import type {ComponentGallery} from "./types"

const WALLET_ADDRESS = "0:4f3d8fd7bb90f65b46c55cbfc783963f7ec87b8c4e5955666fca73c501cdab10"
const TREASURY_ADDRESS = "0:7c669d1ed76614c57a6236981f4da0dc9753591af844235f666e7239d635f52d"
const OLD_CELL_BOC = "b5ee9c7201010101000a00001073656e64"
const NEW_CELL_BOC = "b5ee9c7201010101000b00001272656365697665"

const contracts = new Map<string, ContractChipData>([
  [WALLET_ADDRESS, {displayName: "Wallet V4", letter: "W"}],
  [TREASURY_ADDRESS, {displayName: "Treasury", letter: "T"}],
])

const unchangedDiff: ParsedValueDiff = {
  kind: "leaf",
  status: "unchanged",
  before: {kind: "scalar", value: "42"},
  after: {kind: "scalar", value: "42"},
}

const changedDiff: ParsedValueDiff = {
  kind: "leaf",
  status: "changed",
  before: {kind: "scalar", value: "1000000000", typeName: "coins"},
  after: {kind: "scalar", value: "1750000000", typeName: "coins"},
}

const addedDiff: ParsedValueDiff = {
  kind: "leaf",
  status: "added",
  before: undefined,
  after: {kind: "boolean", value: true},
}

const removedDiff: ParsedValueDiff = {
  kind: "leaf",
  status: "removed",
  before: {kind: "scalar", value: "legacy"},
  after: undefined,
}

const addressDiff: ParsedValueDiff = {
  kind: "leaf",
  status: "changed",
  before: {kind: "address", value: WALLET_ADDRESS},
  after: {kind: "address", value: TREASURY_ADDRESS},
}

const cellDiff: ParsedValueDiff = {
  kind: "leaf",
  status: "changed",
  before: {
    kind: "scalar",
    value: "Cell(b5ee9c720101…1073656e64)",
    rawValue: OLD_CELL_BOC,
  },
  after: {
    kind: "scalar",
    value: "Cell(b5ee9c720101…72656365697665)",
    rawValue: NEW_CELL_BOC,
  },
}

const gramDiff: ParsedValueDiff = {
  kind: "leaf",
  status: "changed",
  before: {kind: "scalar", value: "95000000", typeName: "coins"},
  after: {kind: "scalar", value: "120000000", typeName: "coins"},
}

const beforeStorage: ParsedStorageValue = {
  name: "AccountState",
  value: {
    kind: "object",
    typeName: "AccountState",
    entries: [
      {key: "owner", value: {kind: "address", value: WALLET_ADDRESS}},
      {key: "balance", value: {kind: "scalar", value: "1000000000", typeName: "coins"}},
      {key: "active", value: {kind: "boolean", value: true}},
      {key: "nonce", value: {kind: "scalar", value: "7"}},
      {
        key: "limits",
        value: {
          kind: "array",
          items: [
            {kind: "scalar", value: "10"},
            {kind: "scalar", value: "20"},
          ],
        },
      },
      {
        key: "permissions",
        value: {
          kind: "map",
          typeName: "map<int, bool>",
          entries: [
            {key: {kind: "scalar", value: "1"}, value: {kind: "boolean", value: true}},
            {key: {kind: "scalar", value: "2"}, value: {kind: "boolean", value: false}},
          ],
        },
      },
    ],
  },
}

const afterStorage: ParsedStorageValue = {
  name: "AccountState",
  value: {
    kind: "object",
    typeName: "AccountState",
    entries: [
      {key: "owner", value: {kind: "address", value: WALLET_ADDRESS}},
      {key: "balance", value: {kind: "scalar", value: "1750000000", typeName: "coins"}},
      {key: "active", value: {kind: "boolean", value: false}},
      {key: "createdAt", value: {kind: "scalar", value: "1936289396"}},
      {
        key: "limits",
        value: {
          kind: "array",
          items: [
            {kind: "scalar", value: "10"},
            {kind: "scalar", value: "25"},
            {kind: "scalar", value: "30"},
          ],
        },
      },
      {
        key: "permissions",
        value: {
          kind: "map",
          typeName: "map<int, bool>",
          entries: [
            {key: {kind: "scalar", value: "1"}, value: {kind: "boolean", value: false}},
            {key: {kind: "scalar", value: "3"}, value: {kind: "boolean", value: true}},
          ],
        },
      },
    ],
  },
}

function requiredStorageDiff(
  before: ParsedStorageValue | undefined,
  after: ParsedStorageValue | undefined,
): ParsedValueDiff {
  const diff = buildStorageDiff(before, after)
  if (!diff) throw new Error("Gallery storage values must produce a diff")
  return diff
}

const nestedStorageDiff = requiredStorageDiff(beforeStorage, afterStorage)
const emptyMapDiff = requiredStorageDiff(
  {name: "Registry", value: {kind: "map", typeName: "map<int, address>", entries: []}},
  {name: "Registry", value: {kind: "map", typeName: "map<int, address>", entries: []}},
)
const emptyArrayDiff = requiredStorageDiff(
  {name: "RecentValues", value: {kind: "array", items: []}},
  {name: "RecentValues", value: {kind: "array", items: []}},
)
const changedMapValueDiff = requiredStorageDiff(
  {
    name: "Permissions",
    value: {
      kind: "map",
      typeName: "map<int, bool>",
      entries: [{key: {kind: "scalar", value: "1"}, value: {kind: "boolean", value: true}}],
    },
  },
  {
    name: "Permissions",
    value: {
      kind: "map",
      typeName: "map<int, bool>",
      entries: [{key: {kind: "scalar", value: "1"}, value: {kind: "boolean", value: false}}],
    },
  },
)
const changedMapKeyDiff = requiredStorageDiff(
  {
    name: "Permissions",
    value: {
      kind: "map",
      typeName: "map<int, bool>",
      entries: [{key: {kind: "scalar", value: "2"}, value: {kind: "boolean", value: true}}],
    },
  },
  {
    name: "Permissions",
    value: {
      kind: "map",
      typeName: "map<int, bool>",
      entries: [{key: {kind: "scalar", value: "3"}, value: {kind: "boolean", value: true}}],
    },
  },
)
const lastMapEntryRemovedDiff = requiredStorageDiff(
  {
    name: "Permissions",
    value: {
      kind: "map",
      typeName: "map<int, bool>",
      entries: [{key: {kind: "scalar", value: "7"}, value: {kind: "boolean", value: true}}],
    },
  },
  {
    name: "Permissions",
    value: {kind: "map", typeName: "map<int, bool>", entries: []},
  },
)
const lastArrayItemRemovedDiff = requiredStorageDiff(
  {
    name: "RecentValues",
    value: {kind: "array", items: [{kind: "scalar", value: "42"}]},
  },
  {
    name: "RecentValues",
    value: {kind: "array", items: []},
  },
)

function diffSample(label: string, diff: ParsedValueDiff, fieldName?: string) {
  return (
    <article className={styles.sample}>
      <span className={styles.sampleLabel}>{label}</span>
      <div className={styles.sampleValue}>
        <ParsedValueDiffView diff={diff} contracts={contracts} fieldName={fieldName} />
      </div>
    </article>
  )
}

export const parsedValueDiffViewGallery = {
  id: "parsed-value-diff-view",
  title: "ParsedValueDiffView",
  status: "ready",
  summary:
    "ParsedValueDiffView renders presentation-ready value diffs, while buildStorageDiff compares the same ABI-independent ParsedValue model.",
  importStatement: 'import {buildStorageDiff, ParsedValueDiffView} from "@acton/ui"',
  agentSummary:
    "Use buildStorageDiff for named before/after ParsedValue roots, then pass the result to ParsedValueDiffView. Both APIs stay independent of TON cells, ABI symbols, and parser implementations.",
  usage: [
    "Build a diff from two minimal ParsedStorageValue objects or provide a ParsedValueDiff directly.",
    "Pass ContractChip metadata and address formatting options when diff leaves contain addresses.",
    "Use rawValue on serialized scalar leaves to retain the shared copy action.",
  ],
  avoid: [
    "Do not pass ABI objects, cells, dictionaries, or parser contexts into the component.",
    "Do not compare rendered strings in callers; use buildStorageDiff on ParsedValue trees.",
    "Do not duplicate scalar, address, or copy rendering for diff leaves.",
  ],
  sections: [
    {
      id: "parsed-value-diff-statuses",
      title: "Leaf Statuses",
      description: "Unchanged, changed, added, and removed values exercise every diff status.",
      content: (
        <div className={styles.grid}>
          {diffSample("unchanged", unchangedDiff)}
          {diffSample("changed", changedDiff)}
          {diffSample("added", addedDiff)}
          {diffSample("removed", removedDiff)}
        </div>
      ),
    },
    {
      id: "parsed-value-diff-rich-leaves",
      title: "Address and Cell-like Values",
      description:
        "Rich leaves reuse ContractChip, scalar formatting, and the compact raw-value copy action.",
      content: (
        <div className={styles.grid}>
          {diffSample("address changed", addressDiff)}
          {diffSample("serialized Cell changed", cellDiff)}
          {diffSample("GRAM value changed", gramDiff, "storageGrams")}
        </div>
      ),
    },
    {
      id: "parsed-value-diff-maps",
      title: "Map Changes",
      description:
        "A stable key changes only its value, while replacing a key renders one removed pair and one added pair.",
      content: (
        <div className={styles.grid}>
          {diffSample("value changed", changedMapValueDiff)}
          {diffSample("key replaced", changedMapKeyDiff)}
          {diffSample("last map entry removed", lastMapEntryRemovedDiff)}
          {diffSample("last array item removed", lastArrayItemRemovedDiff)}
        </div>
      ),
    },
    {
      id: "parsed-value-diff-storage",
      title: "Built Storage Diff",
      description:
        "A single builder call compares nested objects, arrays, maps, additions, removals, and unchanged leaves.",
      content: (
        <div className={styles.structure}>
          <ParsedValueDiffView diff={nestedStorageDiff} contracts={contracts} />
        </div>
      ),
    },
    {
      id: "parsed-value-diff-empty",
      title: "Empty Containers",
      description:
        "Unchanged empty maps and arrays retain their type and use their native empty literal.",
      content: (
        <div className={styles.grid}>
          {diffSample("empty map", emptyMapDiff)}
          {diffSample("empty array", emptyArrayDiff)}
        </div>
      ),
    },
  ],
} satisfies ComponentGallery
