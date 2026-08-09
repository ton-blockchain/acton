import {MultiValueInput} from "@acton/ui"
import {useState} from "react"

import styles from "./multiValueInputGallery.module.css"
import type {ComponentGallery} from "./types"

const walletNames = ["deployer", "treasury", "testnet-deployer", "vesting-admin"] as const

function InteractiveSample() {
  const [values, setValues] = useState<readonly string[]>(["deployer"])

  return (
    <div className={styles.stack}>
      <MultiValueInput
        label="Startup accounts"
        description="Wallets to create when the environment starts"
        placeholder="Search wallets"
        values={values}
        options={walletNames}
        onValuesChange={setValues}
      />
    </div>
  )
}

function StateSamples() {
  const [emptyValues, setEmptyValues] = useState<readonly string[]>([])

  return (
    <div className={styles.grid}>
      <MultiValueInput
        label="Required"
        placeholder="Select accounts"
        required
        values={emptyValues}
        options={walletNames}
        onValuesChange={setEmptyValues}
      />
      <MultiValueInput
        label="Invalid"
        invalid
        values={["unknown-wallet"]}
        options={walletNames}
        onValuesChange={() => undefined}
      />
      <MultiValueInput
        label="Disabled"
        disabled
        values={["deployer", "treasury"]}
        options={walletNames}
        onValuesChange={() => undefined}
      />
    </div>
  )
}

export const multiValueInputGallery = {
  id: "multi-value-input",
  title: "MultiValueInput",
  status: "ready",
  summary:
    "MultiValueInput selects several strings from a searchable option set and keeps them as removable chips.",
  importStatement: 'import { MultiValueInput } from "@acton/ui"',
  agentSummary:
    "Use MultiValueInput for controlled multi-select fields backed by a finite caller-provided option list.",
  usage: [
    "Keep selected values as an array and pass the complete list through onValuesChange.",
    "Use for known values such as workspace wallets, environments, or presets.",
    "Let the component own keyboard navigation, chip removal, and suggestion placement.",
  ],
  avoid: [
    "Do not serialize selections into comma-separated display text.",
    "Do not use for arbitrary free-form tags.",
    "Do not add a separate list or chip row around the component.",
  ],
  sections: [
    {
      id: "multi-value-input-interactive",
      title: "Interactive",
      description: "Filter options, add chips, and remove them with the pointer or keyboard.",
      content: <InteractiveSample />,
    },
    {
      id: "multi-value-input-states",
      title: "States",
      description: "Required, invalid, and disabled field states.",
      content: <StateSamples />,
    },
  ],
} satisfies ComponentGallery
