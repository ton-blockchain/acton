import {
  ACTON_BUTTON_VARIANTS,
  Button,
  type ActonButtonSize,
  type ActonButtonVariant,
} from "@acton/ui"
import {ArrowRight, Circle} from "lucide-react"

import styles from "./buttonGallery.module.css"
import type {ComponentGallery} from "./types"

const buttonVariants = Object.keys(ACTON_BUTTON_VARIANTS.variant) as ActonButtonVariant[]
const buttonSizes = Object.keys(ACTON_BUTTON_VARIANTS.size) as ActonButtonSize[]

const variantUse = {
  primary: "The main inverted-neutral action in a focused flow.",
  secondary: "The default action when no choice should dominate.",
  outline: "A low-emphasis action that still needs a clear boundary.",
  ghost: "Low-emphasis toolbar and compact repeated controls.",
  danger: "Destructive actions with clear intent or confirmation nearby.",
} satisfies Record<ActonButtonVariant, string>

const sizeUse = {
  sm: "Dense toolbars and table rows.",
  md: "Default forms, panels, and app controls.",
  lg: "Primary actions in sparse layouts.",
  icon: "Icon-only toolbar actions with an accessible label.",
} satisfies Record<ActonButtonSize, string>

const iconProps = {
  size: 16,
  strokeWidth: 2.25,
} as const

function DotIcon() {
  return <Circle {...iconProps} aria-hidden="true" />
}

function ArrowIcon() {
  return <ArrowRight {...iconProps} aria-hidden="true" />
}

function VariantMatrix() {
  return (
    <div className={styles.grid}>
      {buttonVariants.map(variant => (
        <article key={variant} className={styles.sample}>
          <div className={styles.sampleText}>
            <h4>{variant}</h4>
            <p>{variantUse[variant]}</p>
          </div>
          <Button variant={variant}>Action</Button>
        </article>
      ))}
    </div>
  )
}

function SizeScale() {
  return (
    <div className={styles.grid}>
      {buttonSizes.map(size => (
        <article key={size} className={styles.sample}>
          <div className={styles.sampleText}>
            <h4>{size}</h4>
            <p>{sizeUse[size]}</p>
          </div>
          <Button
            size={size}
            aria-label={size === "icon" ? "Icon action" : undefined}
            title={size === "icon" ? "Icon action" : undefined}
          >
            {size === "icon" ? <DotIcon /> : "Action"}
          </Button>
        </article>
      ))}
    </div>
  )
}

function ContentPatterns() {
  return (
    <div className={styles.inlineSamples}>
      <Button>Text only</Button>
      <Button leadingIcon={<DotIcon />}>Leading icon</Button>
      <Button trailingIcon={<ArrowIcon />}>Trailing icon</Button>
      <Button size="icon" aria-label="Icon-only action" title="Icon-only action">
        <DotIcon />
      </Button>
    </div>
  )
}

function StateSamples() {
  return (
    <div className={styles.inlineSamples}>
      <Button>Default</Button>
      <Button disabled>Disabled</Button>
      <Button loading>Loading</Button>
      <Button variant="danger" disabled>
        Disabled danger
      </Button>
    </div>
  )
}

export const buttonGallery = {
  id: "button",
  title: "Button",
  status: "ready",
  summary:
    "Button is the baseline action control for Acton interfaces. Use it for explicit user-triggered actions, not navigation or passive labels.",
  importStatement: 'import { Button } from "@acton/ui"',
  agentSummary:
    "Prefer Button for command actions. Pick one primary action per local decision area, use secondary as the default neutral style, and reserve danger for destructive operations.",
  usage: [
    "Use for actions that submit, confirm, start, stop, create, delete, or change state.",
    "Keep labels short and action-oriented.",
    "Use exactly one primary button in a local action group.",
  ],
  avoid: [
    "Do not use Button for links to another route; use a link component when navigation is the behavior.",
    "Do not use danger for reversible or harmless actions.",
    "Do not mix several high-emphasis variants in the same compact group.",
  ],
  sections: [
    {
      id: "button-variants",
      title: "Variants",
      description: "All visual variants shown with their intended hierarchy.",
      content: <VariantMatrix />,
    },
    {
      id: "button-sizes",
      title: "Sizes",
      description: "The available sizing scale for app controls and toolbar actions.",
      content: <SizeScale />,
    },
    {
      id: "button-content",
      title: "Content Patterns",
      description: "Supported label, icon, and icon-only compositions.",
      content: <ContentPatterns />,
    },
    {
      id: "button-states",
      title: "States",
      description: "Common interaction states that should stay visually distinct.",
      content: <StateSamples />,
    },
  ],
} satisfies ComponentGallery
