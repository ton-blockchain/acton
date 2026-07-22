import {createElement} from "react"

import {DialogGallerySamples} from "./dialogGallerySamples"
import type {ComponentGallery} from "./types"

export const dialogGallery = {
  id: "dialog",
  title: "Dialog",
  status: "ready",
  summary:
    "Dialog provides the shared modal frame, backdrop, focus management, dismissal behavior, and viewport-safe scrolling.",
  importStatement: 'import {Dialog} from "@acton/ui"',
  agentSummary:
    "Use Dialog for modal inspection and focused workflows. Keep domain content caller-owned and compose existing UI components inside it.",
  usage: [
    "Use for content that must trap focus and temporarily block interaction with the page.",
    "Use title as the required accessible dialog name and description for optional supporting context.",
    "Compose RawDataBlock, DataTable, AddressChip, and other domain components inside the shared frame.",
    "Let onOpenChange handle close button, Escape, and outside-press state changes.",
  ],
  avoid: [
    "Do not rebuild fixed overlays, backdrops, Escape listeners, or close buttons locally.",
    "Do not use Dialog for compact context that belongs in a Popover.",
    "Do not add a second scroll container around the shared dialog content.",
  ],
  sections: [
    {
      id: "dialog-states",
      title: "Display States",
      description:
        "Standard and long-content dialogs exercise composition, dismissal, focus management, and viewport scrolling.",
      content: createElement(DialogGallerySamples),
    },
  ],
} satisfies ComponentGallery
