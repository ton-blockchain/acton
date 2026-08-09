import {GramAmount} from "@acton/ui"

import styles from "./gramAmountGallery.module.css"
import type {ComponentGallery} from "./types"

function GramAmountSamples() {
  return (
    <div className={styles.grid}>
      {[
        ["Exact", <GramAmount key="exact" value={1_234_567_890n} />],
        ["Grouped", <GramAmount key="grouped" value={1_234_567_890_000_000n} useGrouping />],
        [
          "Compact",
          <GramAmount key="compact" value={1n} maximumFractionDigits={4} showLessThanMinimum />,
        ],
        ["Signed", <GramAmount key="signed" value={250_000_000n} signDisplay="always" />],
      ].map(([label, value]) => (
        <div className={styles.item} key={label as string}>
          <span>{label}</span>
          <strong>{value}</strong>
        </div>
      ))}
    </div>
  )
}

export const gramAmountGallery = {
  id: "gram-amount",
  title: "GramAmount",
  status: "ready",
  summary: "GramAmount renders exact nanogram values as consistent GRAM amounts",
  importStatement: 'import {GramAmount, formatGramAmount} from "@acton/ui"',
  agentSummary:
    "Use GramAmount for rendered GRAM values and formatGramAmount only where JSX is unavailable",
  usage: [
    "Pass integer nanograms as bigint, safe integer, or integer string",
    "Keep the tooltip enabled when the visible value is rounded or abbreviated",
    "Use maximumFractionDigits to preserve an existing compact presentation",
    "Use showUnit false only for editable numeric fields that provide their own GRAM suffix",
  ],
  avoid: [
    "Do not convert nanograms through Number before formatting",
    "Do not append GRAM to a separately formatted decimal string",
    "Do not create local GRAM or nanogram formatters",
  ],
  sections: [
    {
      id: "gram-amount-formats",
      title: "Formats",
      description: "Every format keeps the original nanogram value available in the tooltip",
      content: <GramAmountSamples />,
    },
  ],
} satisfies ComponentGallery
