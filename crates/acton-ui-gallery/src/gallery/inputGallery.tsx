import {Input} from "@acton/ui"
import {KeyRound, Search} from "lucide-react"
import {useState} from "react"

import styles from "./inputGallery.module.css"
import type {ComponentGallery} from "./types"

function SizeSamples() {
  return (
    <div className={styles.sizeGrid}>
      <Input size="sm" aria-label="Small input" placeholder="Small" />
      <Input size="md" aria-label="Medium input" placeholder="Medium" />
      <Input size="lg" aria-label="Large input" placeholder="Large" />
    </div>
  )
}

function StateSamples() {
  return (
    <div className={styles.grid}>
      <Input aria-label="Default input" placeholder="Default" />
      <Input aria-label="Filled input" defaultValue="EQD36X...ur8XSS" />
      <Input aria-label="Read-only input" defaultValue="Read-only value" readOnly />
      <Input aria-label="Disabled input" placeholder="Disabled" disabled />
      <Input aria-label="Invalid input" defaultValue="invalid address" invalid />
      <Input aria-label="Required input" placeholder="Required" required />
    </div>
  )
}

function FieldSamples() {
  return (
    <div className={styles.grid}>
      <Input label="Contract name" placeholder="JettonWallet" />
      <Input
        label="Endpoint"
        description="HTTPS Toncenter V3 endpoint used by the explorer."
        placeholder="https://example.com/api/v3"
        required
      />
    </div>
  )
}

function TechnicalSamples() {
  const [value, setValue] = useState("1000")

  return (
    <div className={styles.grid}>
      <Input
        type="search"
        aria-label="Search contracts"
        placeholder="Search contracts"
        leadingIcon={<Search size={16} />}
      />
      <Input
        type="password"
        label="API key"
        placeholder="Optional API key"
        leadingIcon={<KeyRound size={16} />}
      />
      <Input
        type="number"
        label="Amount"
        min="0"
        inputMode="decimal"
        value={value}
        onChange={event => setValue(event.target.value)}
      />
      <Input mono label="Code hash" defaultValue="b5ee9c720101040100340001..." spellCheck={false} />
    </div>
  )
}

export const inputGallery = {
  id: "input",
  title: "Input",
  status: "ready",
  summary:
    "Input is the shared single-line text control for forms, filters, and editable technical values.",
  importStatement: 'import { Input } from "@acton/ui"',
  agentSummary:
    "Use Input for standalone single-line values. Use label and description for a self-contained field, invalid for validation state, and mono for hashes or addresses.",
  usage: [
    "Use sm, md, and lg to match the density of the surrounding form.",
    "Use label and description when the field does not already have an external label.",
    "Use invalid or aria-invalid for validation styling; report the actual failure through Toast.",
    "Use mono for code hashes, raw addresses, and other fixed-width technical values.",
    "Autocomplete, autocorrect, capitalization, and spellcheck default to off; opt in for human-language fields.",
  ],
  avoid: [
    "Do not use Input for multiline content, file uploads, checkboxes, or selects.",
    "Do not add icons, suffix buttons, or unit labels with absolute positioning inside Input; use a dedicated composite control.",
    "Do not render validation or request error messages inside Input; use Toast and preserve aria-invalid.",
  ],
  sections: [
    {
      id: "input-sizes",
      title: "Sizes",
      description: "Three stable heights cover dense toolbars, regular forms, and roomy dialogs.",
      content: <SizeSamples />,
    },
    {
      id: "input-states",
      title: "States",
      description: "Default, filled, read-only, disabled, invalid, and required native states.",
      content: <StateSamples />,
    },
    {
      id: "input-fields",
      title: "Field Composition",
      description:
        "Label and description are optional. Without them, Input renders only the native input element.",
      content: <FieldSamples />,
    },
    {
      id: "input-technical",
      title: "Technical Values",
      description:
        "Search, password, number, and monospace value scenarios used across Acton tools.",
      content: <TechnicalSamples />,
    },
  ],
} satisfies ComponentGallery
