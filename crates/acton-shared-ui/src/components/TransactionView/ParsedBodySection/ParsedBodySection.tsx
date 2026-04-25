import * as React from "react"
import {useState} from "react"
import {FiChevronDown, FiChevronUp} from "react-icons/fi"

import type {ContractData, ParsedTransactionBody} from "@/types/transaction"

import {ParsedValueView} from "../ParsedValueView/ParsedValueView"

import styles from "./ParsedBodySection.module.css"

interface ParsedBodySectionProps {
  readonly parsedBody: ParsedTransactionBody | undefined
  readonly contracts: Map<string, ContractData>
  readonly onContractClick?: (address: string) => void
  readonly defaultExpanded?: boolean
}

export function ParsedBodySection({
  parsedBody,
  contracts,
  onContractClick,
  defaultExpanded = false,
}: ParsedBodySectionProps): React.JSX.Element | undefined {
  const [isExpanded, setIsExpanded] = useState(defaultExpanded)

  if (!parsedBody) {
    return
  }

  return (
    <div className={styles.parsedBodySection}>
      <div className={styles.parsedBodyTitle}>
        Parsed Body
        <button
          type="button"
          onClick={() => {
            setIsExpanded(!isExpanded)
          }}
          className={styles.actionsToggleButton}
          aria-label={isExpanded ? "Hide parsed body" : "Show parsed body"}
        >
          {isExpanded ? <FiChevronUp size={14} /> : <FiChevronDown size={14} />}
          <span className={styles.actionsToggleText}>{isExpanded ? "Hide" : "Show"}</span>
        </button>
      </div>
      {isExpanded && (
        <div className={styles.parsedBodyTree}>
          <div className={styles.parsedBodyContent}>
            <ParsedValueView
              value={parsedBody.value}
              contracts={contracts}
              onContractClick={onContractClick}
              fallbackTypeName={parsedBody.name}
            />
          </div>
        </div>
      )}
    </div>
  )
}
