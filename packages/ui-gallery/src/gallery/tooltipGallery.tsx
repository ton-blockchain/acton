import {Button, Tooltip, type TooltipPlacement} from "@acton/ui"
import {Info} from "lucide-react"

import styles from "./tooltipGallery.module.css"
import type {ComponentGallery} from "./types"

const placements = ["top", "right", "bottom", "left"] satisfies readonly TooltipPlacement[]

function ButtonIntegrationSamples() {
  return (
    <div className={styles.inlineSamples}>
      <Button title="Run transaction emulation">Emulate</Button>
      <Button
        size="icon"
        aria-label="Show transaction information"
        title="Show transaction information"
      >
        <Info size={16} aria-hidden="true" />
      </Button>
    </div>
  )
}

function PlacementSamples() {
  return (
    <div className={styles.placementGrid}>
      {placements.map(placement => (
        <Tooltip
          key={placement}
          content={`${placement[0]?.toUpperCase()}${placement.slice(1)} placement`}
          placement={placement}
        >
          <Button size="sm">{placement}</Button>
        </Tooltip>
      ))}
    </div>
  )
}

function DirectCompositionSample() {
  return (
    <div className={styles.inlineSamples}>
      <Tooltip content="A longer explanation can wrap without widening the surrounding layout.">
        <Button variant="outline">Hover or focus</Button>
      </Tooltip>
      <Tooltip content="This tooltip opens immediately." delay={0}>
        <Button variant="ghost">No delay</Button>
      </Tooltip>
    </div>
  )
}

export const tooltipGallery = {
  id: "tooltip",
  title: "Tooltip",
  status: "ready",
  summary:
    "Tooltip adds a styled, collision-aware label to buttons on hover or keyboard focus without leaving a native title attribute in the DOM.",
  importStatement: 'import {Tooltip} from "@acton/ui"',
  agentSummary:
    "Use Tooltip only for concise supplementary button labels. Shared Button, InlineButton, and InlineAction already convert their title prop into this tooltip.",
  usage: [
    "Use the title prop on shared button components for the common case.",
    "Compose Tooltip directly around a button when placement or timing must be customized.",
    "Keep an aria-label on icon-only buttons; the tooltip is not their accessible name.",
  ],
  avoid: [
    "Do not use Tooltip for essential instructions, errors, or interactive content.",
    "Do not wrap links and passive text until their tooltip behavior is designed separately.",
    "Do not put another interactive element inside tooltip content.",
  ],
  sections: [
    {
      id: "tooltip-button-integration",
      title: "Button Integration",
      description: "Hover the buttons or focus them with the keyboard to reveal their labels.",
      content: <ButtonIntegrationSamples />,
    },
    {
      id: "tooltip-placement",
      title: "Placement",
      description: "The popup stays in a portal and flips when its requested side has no room.",
      content: <PlacementSamples />,
    },
    {
      id: "tooltip-direct-composition",
      title: "Direct Composition",
      description: "Use the component directly for custom content, delay, or placement.",
      content: <DirectCompositionSample />,
    },
  ],
} satisfies ComponentGallery
