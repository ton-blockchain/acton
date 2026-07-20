import {InfoPopover, InlineButton, MarkdownText} from "@acton/ui"
import {ExternalLink} from "lucide-react"

import styles from "./infoPopoverGallery.module.css"
import type {ComponentGallery} from "./types"

function ContractTypeSample() {
  return (
    <div className={styles.samplePanel}>
      <div className={styles.infoRow}>
        <span className={styles.rowLabel}>Contract type</span>
        <span className={styles.rowValue}>
          <span>Wallet V5R1</span>
          <InfoPopover id="info-popover-contract" ariaLabel="Show contract description">
            <>
              <span className={styles.popoverTitle}>Wallet V5R1</span>
              <span className={styles.popoverText}>
                Standard wallet contract with extension support and signed external messages.
              </span>
              <span className={styles.linkList}>
                <a
                  className={styles.popoverLink}
                  href="https://docs.ton.org/"
                  rel="noreferrer"
                  target="_blank"
                >
                  <span>TON docs</span>
                  <ExternalLink size={13} aria-hidden="true" />
                </a>
              </span>
            </>
          </InfoPopover>
        </span>
      </div>
      <div className={styles.infoRow}>
        <span className={styles.rowLabel}>Status</span>
        <span className={styles.rowValue}>
          <span>Active</span>
          <InfoPopover id="info-popover-status" ariaLabel="Show status details" placement="top">
            <>
              <span className={styles.popoverTitle}>Active account</span>
              <span className={styles.popoverText}>
                The account has initialized state and can process inbound messages.
              </span>
            </>
          </InfoPopover>
        </span>
      </div>
    </div>
  )
}

function ClickHelpSample() {
  return (
    <div className={styles.samplePanel}>
      <div className={styles.infoRow}>
        <span className={styles.rowLabel}>Send mode</span>
        <span className={styles.rowValue}>
          <span>3</span>
          <InfoPopover
            id="info-popover-send-mode"
            ariaLabel="Show send mode details"
            interaction="click"
          >
            <>
              <span className={styles.popoverTitle}>Send mode 3</span>
              <MarkdownText tone="muted">
                Pays fees separately and ignores selected delivery errors. Use this when the
                explanation needs `inline code`, links, or several lines.
              </MarkdownText>
              <InlineButton variant="utility" trailingIcon={<ExternalLink size={13} />}>
                Open docs
              </InlineButton>
            </>
          </InfoPopover>
        </span>
      </div>
    </div>
  )
}

export const infoPopoverGallery = {
  id: "info-popover",
  title: "InfoPopover",
  status: "ready",
  summary: "InfoPopover renders the standard compact info icon trigger for rich contextual help.",
  importStatement: 'import {InfoPopover} from "@acton/ui"',
  agentSummary:
    "Use InfoPopover for the standard inline info icon attached to labels or values. Use Popover directly when the trigger is custom text, a badge, or an action.",
  usage: [
    "Use next to dense technical labels or values when the UI needs a small help affordance.",
    "Keep the explanation content in the caller; InfoPopover only owns the trigger and popover wiring.",
    'Use interaction="click" when users need to click links or actions inside the panel.',
    "Pass id when another label or row should reference the popover panel.",
  ],
  avoid: [
    "Do not use InfoPopover for custom text triggers; use Popover directly.",
    "Do not place long documentation, logs, forms, or destructive confirmations inside it.",
    "Do not rebuild a local info-icon popover with ad hoc portal or positioning code.",
  ],
  sections: [
    {
      id: "info-popover-inline",
      title: "Inline Info",
      description:
        "The standard icon sits directly after a technical value and opens a compact explanation.",
      content: <ContractTypeSample />,
    },
    {
      id: "info-popover-click",
      title: "Interactive Content",
      description: "Click interaction keeps links and actions easy to reach inside the popover.",
      content: <ClickHelpSample />,
    },
  ],
} satisfies ComponentGallery
