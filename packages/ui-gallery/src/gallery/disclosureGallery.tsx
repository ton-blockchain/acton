import {Disclosure} from "@acton/ui"

import styles from "./disclosureGallery.module.css"
import type {ComponentGallery} from "./types"

function DisclosureSamples() {
  return (
    <div className={styles.stack}>
      <Disclosure label="Network and mining">
        <div className={styles.content}>Runtime options appear here</div>
      </Disclosure>
      <Disclosure
        label="Fork settings"
        description="Resolve account state from an existing TON network"
        open
      >
        <dl className={styles.values}>
          <dt>Network</dt>
          <dd>Mainnet</dd>
          <dt>Block</dt>
          <dd>81973221</dd>
        </dl>
      </Disclosure>
    </div>
  )
}

function WrappedLabelSample() {
  return (
    <div className={styles.narrow}>
      <Disclosure
        label="Advanced transaction execution settings"
        description="This supporting text wraps without moving or replacing the chevron"
      >
        <div className={styles.content}>Expanded content remains aligned with the label</div>
      </Disclosure>
    </div>
  )
}

export const disclosureGallery = {
  id: "disclosure",
  title: "Disclosure",
  status: "ready",
  summary:
    "Disclosure is the shared full-width collapsible section for forms, settings, and inspection panels.",
  importStatement: 'import { Disclosure } from "@acton/ui"',
  agentSummary:
    "Use Disclosure instead of raw details and summary when a full-width section reveals caller-owned content.",
  usage: [
    "Use for optional form groups, settings, and full-width inspection sections.",
    "Pass description when the consequence of opening the section is not obvious.",
    "Keep domain content inside the component and use contentClassName for its layout.",
  ],
  avoid: [
    "Do not render a browser-native disclosure marker.",
    "Do not add a second chevron or custom open-state rotation.",
    "Do not use for compact inline Show/Hide controls; use DisclosureToggle.",
  ],
  sections: [
    {
      id: "disclosure-states",
      title: "Closed and open",
      description: "Native closed and open states with the shared chevron and spacing.",
      content: <DisclosureSamples />,
    },
    {
      id: "disclosure-wrapped-label",
      title: "Wrapped label",
      description: "A narrow disclosure with a long label and supporting text.",
      content: <WrappedLabelSample />,
    },
  ],
} satisfies ComponentGallery
