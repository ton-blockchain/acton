import {Checkbox} from "@acton/ui"

import styles from "./checkboxGallery.module.css"
import type {ComponentGallery} from "./types"

const states = [
  {
    id: "unchecked",
    title: "Unchecked",
    description: "Default unselected option.",
    checkbox: <Checkbox label="Include empty accounts" />,
  },
  {
    id: "checked",
    title: "Checked",
    description: "Selected option with inverted neutral check fill.",
    checkbox: <Checkbox label="Show internal messages" defaultChecked />,
  },
  {
    id: "disabled",
    title: "Disabled",
    description: "Unavailable option with reduced opacity.",
    checkbox: <Checkbox label="Trace archived state" disabled />,
  },
  {
    id: "disabled-checked",
    title: "Disabled Checked",
    description: "Locked selected option.",
    checkbox: <Checkbox label="Keep latest block pinned" defaultChecked disabled />,
  },
] as const

function StateSamples() {
  return (
    <div className={styles.grid}>
      {states.map(state => (
        <article key={state.id} className={styles.sample}>
          <div className={styles.sampleText}>
            <h4>{state.title}</h4>
            <p>{state.description}</p>
          </div>
          {state.checkbox}
        </article>
      ))}
    </div>
  )
}

function CountSamples() {
  return (
    <div className={styles.filterPanel}>
      <div className={styles.filterHeader}>
        <h4>API call status</h4>
        <span>filter group</span>
      </div>
      <div className={styles.filterGroup}>
        <Checkbox label="Success" count={128} defaultChecked />
        <Checkbox label="Failed" count={7} defaultChecked />
        <Checkbox label="Pending" count={0} />
      </div>
    </div>
  )
}

function DescriptionSamples() {
  return (
    <div className={styles.stack}>
      <Checkbox
        label="Decode message bodies"
        description="Run known ABI decoders when call payloads are available."
        defaultChecked
      />
      <Checkbox
        label="Show low-level VM steps"
        description="Useful while debugging, noisy during normal account review."
      />
    </div>
  )
}

export const checkboxGallery = {
  id: "checkbox",
  title: "Checkbox",
  status: "ready",
  summary:
    "Checkbox is the baseline boolean selection control for filters, preferences, and option lists.",
  importStatement: 'import { Checkbox } from "@acton/ui"',
  agentSummary:
    "Use Checkbox for independent boolean choices. Use count when the label represents a filtered set, such as Success 128 or Failed 7.",
  usage: [
    "Use for independent on/off choices in filters, settings, and option lists.",
    "Use count when the option describes a result set or status bucket.",
    "Use description only when a short label is not enough to make the consequence clear.",
  ],
  avoid: [
    "Do not use Checkbox for mutually exclusive choices; use radios or segmented controls.",
    "Do not use it as a command button.",
    "Do not place long paragraphs inside the label or count slot.",
  ],
  sections: [
    {
      id: "checkbox-states",
      title: "States",
      description:
        "The base checkbox states, including disabled and disabled checked. All check marks here are drawn by Checkbox CSS, without lucide-react.",
      content: <StateSamples />,
    },
    {
      id: "checkbox-counts",
      title: "With Count",
      description: "Counts sit directly after the label for localnet-style filter groups.",
      content: <CountSamples />,
    },
    {
      id: "checkbox-descriptions",
      title: "With Description",
      description: "Optional helper text for settings where the consequence is not obvious.",
      content: <DescriptionSamples />,
    },
  ],
} satisfies ComponentGallery
