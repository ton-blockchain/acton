import React from "react"

import type {StackElement} from "ton-assembly/dist/trace"

import StackViewer from "../stack/StackViewer"
import styles from "./TraceSidePanel.module.css"

export interface TraceSidePanelProps {
  readonly currentStack?: readonly StackElement[]

  readonly onStackItemClick?: (element: StackElement, title: string) => void
  readonly className?: string
}

const TraceSidePanel: React.FC<TraceSidePanelProps> = ({
  currentStack = [],
  onStackItemClick,
  className,
}) => {
  return (
    <div className={`${styles.sidePanel} ${className || ""}`}>
      <div className={styles.stepDetails}>
        <div className={styles.stackViewerContainer}>
          <div className={styles.stackHeader}>
            <span>Stack</span>
          </div>
          <StackViewer stack={currentStack} title="" onStackItemClick={onStackItemClick} />
        </div>
      </div>
    </div>
  )
}

export default TraceSidePanel
