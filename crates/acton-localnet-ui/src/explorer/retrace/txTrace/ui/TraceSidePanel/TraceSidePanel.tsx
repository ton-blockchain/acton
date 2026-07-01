import type {ContractData} from "@acton/shared-ui"

import type {StackElement} from "@ton/tasm/dist/trace"
import React from "react"

import StackViewer from "../stack/StackViewer"
import styles from "./TraceSidePanel.module.css"

export interface TraceSidePanelProps {
  readonly currentStack?: readonly StackElement[]
  readonly contracts?: Map<string, ContractData>

  readonly onStackItemClick?: (element: StackElement, title: string) => void
  readonly className?: string
}

const TraceSidePanel: React.FC<TraceSidePanelProps> = ({
  currentStack = [],
  contracts,
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
          <StackViewer
            stack={currentStack}
            title=""
            contracts={contracts}
            onStackItemClick={onStackItemClick}
          />
        </div>
      </div>
    </div>
  )
}

export default TraceSidePanel
