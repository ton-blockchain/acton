import {IdeSelector, type IdeId} from "@acton/ui"
import {useState} from "react"

import styles from "./ideSelectorGallery.module.css"
import type {ComponentGallery} from "./types"

const sampleLocation = {
  filePath: "/workspace/tests/wallet-behavior.test.tolk",
  line: 37,
  column: 9,
}

function SelectorSample() {
  const [ide, setIde] = useState<IdeId>("RustRover")

  return (
    <div className={styles.sample}>
      <div className={styles.row}>
        <div>
          <p className={styles.label}>Test location</p>
          <p className={styles.path}>wallet-behavior.test.tolk:37:9</p>
        </div>
        <IdeSelector value={ide} onValueChange={setIde} location={sampleLocation} />
      </div>
      <div className={styles.row}>
        <div>
          <p className={styles.label}>Dense code header</p>
          <p className={styles.path}>Compact sizing keeps the file path dominant</p>
        </div>
        <IdeSelector value={ide} onValueChange={setIde} location={sampleLocation} size="compact" />
      </div>
    </div>
  )
}

export const ideSelectorGallery = {
  id: "ide-selector",
  title: "IDE Selector",
  status: "ready",
  summary:
    "IDE Selector combines a direct editor link with an accessible Base UI menu for choosing the preferred IDE.",
  importStatement: 'import {IdeSelector, useIdePreference} from "@acton/ui"',
  agentSummary:
    "Use IdeSelector wherever a source location can be opened in a desktop IDE. Keep the selected IDE shared between instances with useIdePreference.",
  usage: [
    "Pass one-based line and column values in location.",
    "Use the compact size inside dense file and code headers.",
    "Enable the dot shortcut on only one selector per screen.",
  ],
  avoid: [
    "Do not rebuild the IDE menu with local absolute positioning or document event listeners.",
    "Do not enable the same global shortcut on multiple mounted selectors.",
  ],
  sections: [
    {
      id: "ide-selector-sizes",
      title: "Shared Selection",
      description:
        "Both sizes use the same controlled value; changing either menu updates both selectors.",
      content: <SelectorSample />,
    },
  ],
} satisfies ComponentGallery
