import {
  ACTON_INLINE_BUTTON_VARIANTS,
  CopyInlineButton,
  InlineButton,
  type ActonInlineButtonVariant,
} from "@acton/ui"
import {ArrowRight, Bug, Copy, Trash2} from "lucide-react"

import styles from "./inlineButtonGallery.module.css"
import type {ComponentGallery} from "./types"

const inlineButtonVariants = Object.keys(
  ACTON_INLINE_BUTTON_VARIANTS.variant,
) as ActonInlineButtonVariant[]

const variantUse = {
  default: "Neutral embedded action inside dense UI.",
  utility: "Compact copy, reveal, and raw-data utility action.",
  accent: "Debug, inspect, reveal, or related tool action.",
  danger: "Destructive embedded action with local context.",
} satisfies Record<ActonInlineButtonVariant, string>

const iconProps = {
  size: 14,
  strokeWidth: 2.25,
} as const

const utilityIconProps = {
  size: 12,
  strokeWidth: 2,
} as const

function variantIcon(variant: ActonInlineButtonVariant) {
  if (variant === "utility") return <Copy {...utilityIconProps} aria-hidden="true" />
  if (variant === "accent") return <Bug {...iconProps} aria-hidden="true" />
  if (variant === "danger") return <Trash2 {...iconProps} aria-hidden="true" />
  return <ArrowRight {...iconProps} aria-hidden="true" />
}

function VariantSamples() {
  return (
    <div className={styles.grid}>
      {inlineButtonVariants.map(variant => (
        <article key={variant} className={styles.sample}>
          <div className={styles.sampleText}>
            <h4>{variant}</h4>
            <p>{variantUse[variant]}</p>
          </div>
          <div className={styles.sampleAction}>
            <InlineButton variant={variant} leadingIcon={variantIcon(variant)}>
              {variant === "accent" ? "Debug" : variant === "utility" ? "Copy raw body" : "Action"}
            </InlineButton>
          </div>
        </article>
      ))}
    </div>
  )
}

function EmbeddedRow() {
  return (
    <div className={styles.embeddedRow}>
      <span className={styles.rowLabel}>Transaction route</span>
      <span className={styles.rowValue}>{"external-in -> wallet-v5 -> sale"}</span>
      <InlineButton variant="accent" leadingIcon={<Bug {...iconProps} aria-hidden="true" />}>
        Debug
      </InlineButton>
    </div>
  )
}

const rawRows = [
  {
    label: "In message",
    value: "external-in body decoded",
    actions: ["Copy raw message", "Copy raw body", "Copy raw state init"],
  },
  {
    label: "Storage",
    value: "account state diff",
    actions: ["Copy raw storage"],
  },
  {
    label: "Actions",
    value: "2 outbound actions",
    actions: ["Copy raw actions"],
  },
] as const

function UtilityCopyTable() {
  return (
    <table className={styles.utilityTable}>
      <tbody>
        {rawRows.map(row => (
          <tr key={row.label}>
            <th scope="row">{row.label}</th>
            <td>
              <div className={styles.utilityCell}>
                <span className={styles.utilityValue}>{row.value}</span>
                <span className={styles.utilityActions}>
                  {row.actions.map(action => (
                    <CopyInlineButton
                      key={action}
                      value={`${row.label}: ${row.value}`}
                      label={action}
                      copiedLabel={`${action} copied`}
                    >
                      {action}
                    </CopyInlineButton>
                  ))}
                </span>
              </div>
            </td>
          </tr>
        ))}
      </tbody>
    </table>
  )
}

export const inlineButtonGallery = {
  id: "inline-button",
  title: "InlineButton",
  status: "ready",
  summary:
    "InlineButton is an embedded action-link for dense rows, metadata blocks, and tool surfaces. It keeps button semantics without drawing a boxed control.",
  importStatement: 'import { CopyInlineButton, InlineButton } from "@acton/ui"',
  agentSummary:
    "Use InlineButton for Debug-style actions inside existing content. Do not use Button plus custom classes for inline command links.",
  usage: [
    "Use inside rows, cards, details panels, and compact metadata groups.",
    "Use utility for compact copy/reveal/raw-data actions with a text label.",
    "Use CopyInlineButton when a utility copy action needs copied feedback.",
    "Use accent for debug, inspect, reveal, or related tool actions.",
    "Keep the label short and pair with a small lucide icon when the action benefits from recognition.",
  ],
  avoid: [
    "Do not use for primary form actions or standalone footer actions.",
    "Do not add background, border, or fixed control height through className.",
    "Do not use for navigation to another route; use a link component when navigation is the behavior.",
  ],
  sections: [
    {
      id: "inline-button-variants",
      title: "Variants",
      description: "Inline button variants shown without a boxed control surface.",
      content: <VariantSamples />,
    },
    {
      id: "inline-button-context",
      title: "Embedded Context",
      description: "A Debug-style action placed in the kind of dense row where it belongs.",
      content: <EmbeddedRow />,
    },
    {
      id: "inline-button-utility",
      title: "Utility Copy Actions",
      description:
        "Compact copy commands revealed from the table cell that owns the raw technical value.",
      content: <UtilityCopyTable />,
    },
  ],
} satisfies ComponentGallery
