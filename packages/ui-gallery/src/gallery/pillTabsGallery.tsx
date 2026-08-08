import {PillTab, PillTabs, PillTabToggle} from "@acton/ui"
import {useState} from "react"

import styles from "./pillTabsGallery.module.css"
import type {ComponentGallery} from "./types"

type TraceTab = "treasury-1" | "treasury-2" | "trace-3" | "trace-4" | "trace-5" | "trace-6"

const treasuryTraces = [
  {value: "treasury-1", label: "Trace 1"},
  {value: "treasury-2", label: "Trace 2"},
] satisfies readonly {label: string; value: TraceTab}[]

const regularTraces = [
  {value: "trace-3", label: "Trace 3"},
  {value: "trace-4", label: "Trace 4"},
] satisfies readonly {label: string; value: TraceTab}[]

const manyTraces = [
  {value: "trace-3", label: "Trace 3"},
  {value: "trace-4", label: "Trace 4"},
  {value: "trace-5", label: "Trace 5"},
  {value: "trace-6", label: "Trace 6"},
] satisfies readonly {label: string; value: TraceTab}[]

function TraceSelectorSample() {
  const [expanded, setExpanded] = useState(true)
  const [selectedTrace, setSelectedTrace] = useState<TraceTab>("trace-3")

  return (
    <div className={styles.frame}>
      <PillTabs ariaLabel="Trace selector">
        <PillTabToggle expanded={expanded} onClick={() => setExpanded(current => !current)}>
          2 treasury deploys
        </PillTabToggle>
        {expanded &&
          treasuryTraces.map(trace => (
            <PillTab
              key={trace.value}
              variant="muted"
              selected={selectedTrace === trace.value}
              onClick={() => setSelectedTrace(trace.value)}
            >
              {trace.label}
            </PillTab>
          ))}
        {regularTraces.map(trace => (
          <PillTab
            key={trace.value}
            selected={selectedTrace === trace.value}
            onClick={() => setSelectedTrace(trace.value)}
          >
            {trace.label}
          </PillTab>
        ))}
      </PillTabs>
    </div>
  )
}

function OverflowSample() {
  const [selectedTrace, setSelectedTrace] = useState<TraceTab>("trace-4")

  return (
    <div className={`${styles.frame} ${styles.narrowFrame}`}>
      <div className={styles.stack}>
        <PillTabs ariaLabel="Narrow trace selector">
          <PillTabToggle expanded={false}>4 treasury deploys</PillTabToggle>
          {manyTraces.map(trace => (
            <PillTab
              key={trace.value}
              selected={selectedTrace === trace.value}
              onClick={() => setSelectedTrace(trace.value)}
            >
              {trace.label}
            </PillTab>
          ))}
          <PillTab variant="muted" disabled>
            3 traces skipped
          </PillTab>
        </PillTabs>
        <p className={styles.hint}>The row scrolls horizontally instead of wrapping trace chips.</p>
      </div>
    </div>
  )
}

export const pillTabsGallery = {
  id: "pill-tabs",
  title: "PillTabs",
  status: "ready",
  summary:
    "PillTabs renders detached pill-like selectors for traces and other compact item groups without owning the content panel.",
  importStatement: 'import { PillTab, PillTabs, PillTabToggle } from "@acton/ui"',
  agentSummary:
    "Use PillTabs for compact selector rows like Test UI trace selection. Use ContentTabs when tabs are connected to a bordered panel.",
  usage: [
    "Use for trace selectors, compact item filters, and rows with an optional group toggle.",
    "Use PillTabToggle for a collapsible group summary such as treasury deploy traces.",
    'Use variant="muted" for group children, skipped items, or low-emphasis trace tabs.',
    "Keep selected item state in the caller.",
  ],
  avoid: [
    "Do not use for connected tab panels; use ContentTabs instead.",
    "Do not put domain logic such as trace filtering inside PillTabs.",
    "Do not use PillTabToggle as a selected tab; it expands or collapses a group.",
  ],
  sections: [
    {
      id: "pill-tabs-trace-selector",
      title: "Trace Selector",
      description: "Detached trace tabs with a collapsible group summary from Test UI.",
      content: <TraceSelectorSample />,
    },
    {
      id: "pill-tabs-overflow",
      title: "Overflow",
      description: "Narrow selectors keep one row and scroll horizontally.",
      content: <OverflowSample />,
    },
  ],
} satisfies ComponentGallery
