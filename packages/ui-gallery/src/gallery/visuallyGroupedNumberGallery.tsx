import {VisuallyGroupedNumber} from "@acton/ui"

import styles from "./visuallyGroupedNumberGallery.module.css"
import type {ComponentGallery} from "./types"

const scalarRows = [
  {label: "walletId", value: "2147483645"},
  {label: "validUntil", value: "4294967295"},
  {label: "seqno", value: "0"},
] as const

function DenseScalarSample() {
  return (
    <div className={styles.scalarPanel}>
      {scalarRows.map(row => (
        <div className={styles.scalarRow} key={row.label}>
          <span className={styles.scalarLabel}>{row.label}:</span>
          <VisuallyGroupedNumber className={styles.scalarValue} value={row.value} />
        </div>
      ))}
    </div>
  )
}

function NumberFormsSample() {
  return (
    <div className={styles.formsGrid}>
      <div className={styles.formSample}>
        <span className={styles.sampleLabel}>Small</span>
        <VisuallyGroupedNumber className={styles.sampleValue} value="309" />
      </div>
      <div className={styles.formSample}>
        <span className={styles.sampleLabel}>Large</span>
        <VisuallyGroupedNumber className={styles.sampleValue} value="1000000000000" />
      </div>
      <div className={styles.formSample}>
        <span className={styles.sampleLabel}>Signed</span>
        <VisuallyGroupedNumber className={styles.sampleValue} value="-1702392942" />
      </div>
      <div className={styles.formSample}>
        <span className={styles.sampleLabel}>Decimal</span>
        <VisuallyGroupedNumber className={styles.sampleValue} value="1234567.8901" />
      </div>
    </div>
  )
}

function TechnicalValueSample() {
  return (
    <div className={styles.scalarPanel}>
      <div className={styles.scalarRow}>
        <span className={styles.scalarLabel}>decimal:</span>
        <VisuallyGroupedNumber className={styles.scalarValue} value="1936289396" />
      </div>
      <div className={styles.scalarRow}>
        <span className={styles.scalarLabel}>hex:</span>
        <VisuallyGroupedNumber className={styles.hexValue} value="0x73656e642d6d6f6465" />
      </div>
      <div className={styles.scalarRow}>
        <span className={styles.scalarLabel}>hash:</span>
        <VisuallyGroupedNumber
          className={styles.hashValue}
          value="65a184650d89a7a435714780a2f6084"
        />
      </div>
    </div>
  )
}

export const visuallyGroupedNumberGallery = {
  id: "visually-grouped-number",
  title: "VisuallyGroupedNumber",
  status: "ready",
  summary:
    "VisuallyGroupedNumber improves readability of long decimal technical values without changing the underlying text.",
  importStatement: 'import {VisuallyGroupedNumber} from "@acton/ui"',
  agentSummary:
    "Use VisuallyGroupedNumber for decimal scalar values, balances, gas values, ids, and counters when the exact text should remain copyable without inserted separators.",
  usage: [
    "Use for long decimal numbers in parsed values, storage diffs, trace summaries, and dense technical tables.",
    "Pass the already formatted display value; the component only groups plain decimal strings.",
    "Non-decimal strings such as hex, hashes, addresses, and short values render unchanged.",
  ],
  avoid: [
    "Do not use for addresses, hashes, base64, or arbitrary identifiers that need their own truncation or monospace treatment.",
    "Do not use when the visible value should include real locale separators; format that outside the component.",
    "Do not insert spaces into technical values just to create visual groups.",
  ],
  sections: [
    {
      id: "visually-grouped-number-dense",
      title: "Dense Scalars",
      description:
        "Long decimal scalar values gain visual grouping while labels and layout stay caller-owned.",
      content: <DenseScalarSample />,
    },
    {
      id: "visually-grouped-number-forms",
      title: "Number Forms",
      description:
        "Short, signed, large, and fractional decimal values share the same visual grouping rules.",
      content: <NumberFormsSample />,
    },
    {
      id: "visually-grouped-number-technical",
      title: "Technical Strings",
      description:
        "Only plain decimal values are grouped; hex and hash-like strings remain untouched.",
      content: <TechnicalValueSample />,
    },
  ],
} satisfies ComponentGallery
