import {InlineLoader} from "@acton/ui"
import {createElement} from "react"

import styles from "./inlineLoaderGallery.module.css"
import type {ComponentGallery} from "./types"

export const inlineLoaderGallery = {
  id: "inline-loader",
  title: "InlineLoader",
  status: "ready",
  summary:
    "InlineLoader presents an indeterminate operation with an accessible status, optional detail, and reduced-motion behavior.",
  importStatement: 'import { InlineLoader } from "@acton/ui"',
  agentSummary:
    "Use InlineLoader when the destination layout is not yet available and the UI needs a centered indeterminate status instead of a skeleton.",
  usage: [
    "Render it only while the operation is in progress.",
    "Use message for the operation name and subtext for a short expectation-setting detail.",
    "Place it inside a caller-owned region that controls available height and alignment.",
  ],
  avoid: [
    "Do not use it when Skeleton can preserve a known destination layout.",
    "Do not keep it mounted after loading completes.",
    "Do not use it for progress with a measurable percentage.",
  ],
  sections: [
    {
      id: "inline-loader-message",
      title: "Message",
      description: "A compact indeterminate state for lazy editor and panel content.",
      content: createElement(
        "div",
        {className: styles.sample},
        createElement(InlineLoader, {message: "Loading editor"}),
      ),
    },
    {
      id: "inline-loader-subtext",
      title: "Message and Subtext",
      description: "Additional context for operations that may take more than a moment.",
      content: createElement(
        "div",
        {className: styles.sample},
        createElement(InlineLoader, {
          message: "Tracing transaction",
          subtext: "This may take a few moments",
        }),
      ),
    },
  ],
} satisfies ComponentGallery
