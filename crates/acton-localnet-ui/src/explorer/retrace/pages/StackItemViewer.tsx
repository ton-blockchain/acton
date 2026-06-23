import React from "react"
import type {StackElement} from "ton-assembly/dist/trace"

import styles from "@retrace/pages/StackItemViewer.module.css"
import StackItemDetails from "@retrace/ui/StackItemDetails"

interface StackItemViewerProps {
  readonly element: StackElement
  readonly title: string
  readonly onBack: () => void
}

export const StackItemViewer: React.FC<StackItemViewerProps> = ({element, title, onBack}) => {
  return (
    <div className={styles.stackItemViewer}>
      <StackItemDetails itemData={element} title={title} onClose={onBack} />
    </div>
  )
}
