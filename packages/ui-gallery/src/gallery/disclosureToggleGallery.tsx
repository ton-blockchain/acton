import {DisclosureToggle} from "@acton/ui"
import {useState} from "react"

import styles from "./disclosureToggleGallery.module.css"
import type {ComponentGallery} from "./types"

function InlineSectionSamples() {
  const [parsedBodyOpen, setParsedBodyOpen] = useState(false)
  const [stateInitOpen, setStateInitOpen] = useState(true)

  return (
    <div className={styles.stack}>
      <div className={styles.row}>
        <span className={styles.sectionLabel}>Parsed Body</span>
        <DisclosureToggle
          expanded={parsedBodyOpen}
          contextLabel="parsed body"
          onClick={() => setParsedBodyOpen(open => !open)}
        />
      </div>
      {parsedBodyOpen ? (
        <div className={styles.content}>
          <h4 className={styles.contentTitle}>WalletSignedExternalV5r1</h4>
          <dl className={styles.fieldGrid}>
            <dt>walletId:</dt>
            <dd>2 147 483 645</dd>
            <dt>validUntil:</dt>
            <dd>4 294 967 295</dd>
            <dt>seqno:</dt>
            <dd>0</dd>
          </dl>
        </div>
      ) : undefined}

      <div className={styles.row}>
        <span className={styles.sectionLabel}>State Init</span>
        <DisclosureToggle
          expanded={stateInitOpen}
          contextLabel="state init"
          onClick={() => setStateInitOpen(open => !open)}
        />
      </div>
    </div>
  )
}

function ValuePrefixSample() {
  const [actionsOpen, setActionsOpen] = useState(false)

  return (
    <div className={styles.valueBlock}>
      <span className={styles.valueLabel}>Total Actions</span>
      <div className={styles.valueLine}>
        <span className={styles.valuePrefix}>1</span>
        <DisclosureToggle
          expanded={actionsOpen}
          contextLabel="actions"
          onClick={() => setActionsOpen(open => !open)}
        />
      </div>
    </div>
  )
}

function LoadingClickSample() {
  const [isLoaded, setIsLoaded] = useState(false)
  const [isLoading, setIsLoading] = useState(false)

  const handleClick = () => {
    if (isLoaded) {
      setIsLoaded(false)
      return
    }

    setIsLoading(true)
    globalThis.setTimeout(() => {
      setIsLoading(false)
      setIsLoaded(true)
    }, 900)
  }

  return (
    <div className={styles.loadingBlock}>
      <div className={styles.row}>
        <span className={styles.sectionLabel}>Storage</span>
        <DisclosureToggle
          expanded={isLoaded}
          loading={isLoading}
          contextLabel="storage state change"
          showLabel="Load"
          loadingLabel="Loading"
          onClick={handleClick}
        />
      </div>
      {isLoaded ? (
        <div className={styles.content}>
          <h4 className={styles.contentTitle}>Storage Diff</h4>
          <dl className={styles.fieldGrid}>
            <dt>status:</dt>
            <dd>Changed</dd>
            <dt>balance:</dt>
            <dd>2.417 TON</dd>
          </dl>
        </div>
      ) : undefined}
    </div>
  )
}

function StateSamples() {
  return (
    <div className={styles.toolbar}>
      <span className={styles.inlinePair}>
        <span className={styles.sectionLabel}>Code</span>
        <DisclosureToggle expanded={false} contextLabel="state init code" />
      </span>
      <span className={styles.inlinePair}>
        <span className={styles.sectionLabel}>Code</span>
        <DisclosureToggle expanded={true} contextLabel="state init code" />
      </span>
      <span className={styles.inlinePair}>
        <span className={styles.valuePrefix}>2</span>
        <DisclosureToggle expanded={false} contextLabel="actions" loading />
      </span>
      <DisclosureToggle
        expanded={false}
        contextLabel="storage state change"
        showLabel="Load"
        loadingLabel="Loading"
        loading
      />
    </div>
  )
}

export const disclosureToggleGallery = {
  id: "disclosure-toggle",
  title: "DisclosureToggle",
  status: "ready",
  summary:
    "DisclosureToggle is a compact Show/Hide trigger for inline expandable details. It owns button semantics, chevron state, labels, loading state, and aria-expanded.",
  importStatement: 'import { DisclosureToggle } from "@acton/ui"',
  agentSummary:
    "Use DisclosureToggle for compact Show/Hide controls next to labels or inline values. Keep expanded content layout outside the component.",
  usage: [
    "Use after section labels such as Parsed Body, State Init, Code, or Disassembled Code.",
    "Keep labels and values outside the component so local layout stays exact.",
    "Use loading with loadingLabel for async reveal actions such as Load/Loading storage.",
  ],
  avoid: [
    "Do not use for full-width collapsible headers; build a separate section/header component for that.",
    "Do not put expanded content inside DisclosureToggle.",
    "Do not recreate chevron, Show/Hide text, or aria-expanded manually when this component fits.",
  ],
  sections: [
    {
      id: "disclosure-toggle-inline-sections",
      title: "Inline Sections",
      description: "Compact toggles placed directly after muted technical labels.",
      content: <InlineSectionSamples />,
    },
    {
      id: "disclosure-toggle-value-prefix",
      title: "Value Prefix",
      description: "The Total Actions pattern where a numeric value precedes the Show/Hide toggle.",
      content: <ValuePrefixSample />,
    },
    {
      id: "disclosure-toggle-states",
      title: "States",
      description: "Closed, open, value-prefix loading, and custom Load/Loading label states.",
      content: <StateSamples />,
    },
    {
      id: "disclosure-toggle-loading",
      title: "Click Loading",
      description: "An async Load action that switches to Loading before revealing content.",
      content: <LoadingClickSample />,
    },
  ],
} satisfies ComponentGallery
