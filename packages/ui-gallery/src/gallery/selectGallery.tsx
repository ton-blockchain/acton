import {Select} from "@acton/ui"

import styles from "./selectGallery.module.css"
import type {ComponentGallery} from "./types"

const networkOptions = (
  <>
    <option value="mainnet">Mainnet</option>
    <option value="testnet">Testnet</option>
    <option value="sandbox">Sandbox</option>
  </>
)

function SizeSamples() {
  return (
    <div className={styles.sizeGrid}>
      <Select size="sm" aria-label="Small network select" defaultValue="mainnet">
        {networkOptions}
      </Select>
      <Select size="md" aria-label="Medium network select" defaultValue="testnet">
        {networkOptions}
      </Select>
      <Select size="lg" aria-label="Large network select" defaultValue="sandbox">
        {networkOptions}
      </Select>
    </div>
  )
}

function StateSamples() {
  return (
    <div className={styles.grid}>
      <Select
        label="Network"
        description="Network used to resolve the account state."
        defaultValue="mainnet"
      >
        {networkOptions}
      </Select>
      <Select label="ABI source" defaultValue="" required invalid>
        <option value="" disabled>
          Select an ABI
        </option>
        <option value="known">Known contracts</option>
        <option value="custom">Custom ABI</option>
      </Select>
      <Select aria-label="Disabled network select" defaultValue="testnet" disabled>
        {networkOptions}
      </Select>
    </div>
  )
}

export const selectGallery = {
  id: "select",
  title: "Select",
  status: "ready",
  summary:
    "Select is the shared native dropdown for choosing one value from a concise, known option set.",
  importStatement: 'import { Select } from "@acton/ui"',
  agentSummary:
    "Use Select when native option, keyboard, focus, and form semantics fit the choice. Keep option labels concise and use label and description for a self-contained field.",
  usage: [
    "Use sm, md, and lg to match the density of surrounding form controls.",
    "Use native option and optgroup elements as children.",
    "Use invalid or aria-invalid for validation styling.",
  ],
  avoid: [
    "Do not use Select when users need search, rich option content, or multi-step filtering.",
    "Do not position another chevron over the control; Select provides one.",
    "Do not replace native option semantics with clickable div elements.",
  ],
  sections: [
    {
      id: "select-sizes",
      title: "Sizes",
      description: "The control and its chevron scale together at all three shared input heights.",
      content: <SizeSamples />,
    },
    {
      id: "select-states",
      title: "Fields and States",
      description: "Labeled, described, required, invalid, and disabled native select states.",
      content: <StateSamples />,
    },
  ],
} satisfies ComponentGallery
