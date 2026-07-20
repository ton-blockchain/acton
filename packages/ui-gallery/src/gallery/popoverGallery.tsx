import {InlineButton, MarkdownText, Popover} from "@acton/ui"
import {Bug, ExternalLink, Info, ShieldCheck} from "lucide-react"

import styles from "./popoverGallery.module.css"
import type {ComponentGallery} from "./types"

function SendModeContent() {
  return (
    <div className={styles.popoverContent}>
      <p className={styles.popoverTitle}>Send mode 3</p>
      <p className={styles.popoverText}>
        Pays fees separately and ignores selected delivery errors. Keep the meaning next to the
        technical value instead of sending users to a separate help panel.
      </p>
      <a
        className={styles.popoverLink}
        href="https://docs.ton.org/"
        rel="noreferrer"
        target="_blank"
      >
        <span>TON docs</span>
        <ExternalLink size={13} aria-hidden="true" />
      </a>
    </div>
  )
}

function StatusContent() {
  return (
    <div className={styles.popoverContent}>
      <div className={styles.statusHeader}>
        <ShieldCheck size={16} aria-hidden="true" />
        <span>Safe to retry</span>
      </div>
      <MarkdownText tone="muted">
        {
          "The previous request reached the wallet, but the dapp has not observed the final trace yet. Retry is idempotent for this session."
        }
      </MarkdownText>
      <div className={styles.contentActions}>
        <InlineButton variant="utility" leadingIcon={<ExternalLink size={13} />}>
          Open trace
        </InlineButton>
      </div>
    </div>
  )
}

function InlineHelpSample() {
  return (
    <div className={styles.samplePanel}>
      <div className={styles.inlineRow}>
        <span className={styles.rowLabel}>Mode</span>
        <Popover
          ariaLabel="Send mode details"
          content={<SendModeContent />}
          placement="top"
          triggerClassName={styles.triggerWrap}
        >
          <span className={styles.inlineTrigger}>send mode 3</span>
        </Popover>
      </div>
      <div className={styles.inlineRow}>
        <span className={styles.rowLabel}>Reserve</span>
        <Popover
          ariaLabel="Reserve mode details"
          content={
            <div className={styles.popoverContent}>
              <p className={styles.popoverTitle}>Reserve exact balance</p>
              <p className={styles.popoverText}>
                Use a popover when the explanation needs links, multiple lines, or structured
                details.
              </p>
            </div>
          }
          placement="right"
          triggerClassName={styles.triggerWrap}
        >
          <span className={styles.inlineTrigger}>exact + bounce-safe</span>
        </Popover>
      </div>
    </div>
  )
}

function ClickPopoverSample() {
  return (
    <div className={styles.samplePanel}>
      <div className={styles.cardHeader}>
        <div>
          <h4 className={styles.sampleTitle}>Session request</h4>
          <p className={styles.sampleText}>
            Click-triggered content may contain buttons and links.
          </p>
        </div>
        <Popover
          ariaLabel="Session request details"
          content={<StatusContent />}
          interaction="click"
          placement="bottom"
        >
          <span className={styles.infoTrigger}>
            <Info size={15} aria-hidden="true" />
            <span>Details</span>
          </span>
        </Popover>
      </div>
      <div className={styles.statusLine}>
        <span className={styles.statusDot} aria-hidden="true" />
        Waiting for wallet approval
      </div>
    </div>
  )
}

function AutoPlacementSample() {
  const placements = ["top", "right", "bottom", "left"] as const

  return (
    <div className={styles.placementGrid}>
      {placements.map(placement => (
        <Popover
          key={placement}
          ariaLabel={`${placement} placement details`}
          content={
            <div className={styles.popoverContent}>
              <p className={styles.popoverTitle}>Preferred {placement}</p>
              <p className={styles.popoverText}>
                The popover starts from this side and flips or shifts when the viewport cannot fit
                it.
              </p>
            </div>
          }
          placement={placement}
        >
          <span className={styles.placementTrigger}>{placement}</span>
        </Popover>
      ))}
    </div>
  )
}

function DenseToolbarSample() {
  return (
    <div className={styles.toolbarSample}>
      <span className={styles.toolbarLabel}>Message body</span>
      <Popover
        ariaLabel="Raw body diagnostics"
        content={
          <div className={styles.popoverContent}>
            <p className={styles.popoverTitle}>Raw body diagnostics</p>
            <p className={styles.popoverText}>
              Popover content can explain why an inline action is available without adding permanent
              text to the row.
            </p>
          </div>
        }
        placement="top"
        tabIndex={-1}
      >
        <InlineButton variant="accent" leadingIcon={<Bug size={14} />}>
          Debug
        </InlineButton>
      </Popover>
    </div>
  )
}

export const popoverGallery = {
  id: "popover",
  title: "Popover",
  status: "ready",
  summary:
    "Popover renders rich contextual overlays for explanations, documentation links, and compact interactive details.",
  importStatement: 'import {Popover} from "@acton/ui"',
  agentSummary:
    "Use Popover when contextual help needs rich content, links, or interactive controls. Prefer hover for inline explanations and click for panels that users need to interact with.",
  usage: [
    "Use for rich contextual help attached to technical values, status labels, and compact toolbar actions.",
    'Use interaction="click" when the panel contains links, buttons, or content users need to inspect deliberately.',
    "Keep domain copy inside the caller; Popover owns trigger wiring, portal rendering, positioning, and overlay styling.",
    "Rely on automatic placement unless a screen has a strong reason to force a side.",
  ],
  avoid: [
    "Do not use Popover for permanent page content or primary workflows.",
    "Do not put long forms, destructive confirmations, or modal flows inside Popover.",
    "Do not rebuild floating help panels with local absolute positioning when Popover fits.",
  ],
  sections: [
    {
      id: "popover-inline-help",
      title: "Inline Help",
      description:
        "Hover and focus reveal rich explanations next to dense technical values without changing row layout.",
      content: <InlineHelpSample />,
    },
    {
      id: "popover-click-content",
      title: "Click Content",
      description:
        "Click-triggered popovers stay open for links, actions, and multi-line status explanations.",
      content: <ClickPopoverSample />,
    },
    {
      id: "popover-auto-placement",
      title: "Auto Placement",
      description:
        "Preferred side is a hint; the component shifts or flips to stay inside the viewport.",
      content: <AutoPlacementSample />,
    },
    {
      id: "popover-toolbar",
      title: "Toolbar Action",
      description:
        "Popover can annotate existing inline actions without changing the action visual style.",
      content: <DenseToolbarSample />,
    },
  ],
} satisfies ComponentGallery
