import {useId, useState} from "react"

import {DisclosureToggle} from "../DisclosureToggle/DisclosureToggle"
import {ParsedValueView, type ParsedValueViewProps} from "../ParsedValueView/ParsedValueView"
import type {ParsedTransactionBody} from "../ParsedValueView/types"

import styles from "./ParsedBodySection.module.css"

export interface ParsedBodySectionProps
  extends Pick<ParsedValueViewProps, "contracts" | "formatAddress" | "onContractClick"> {
  readonly parsedBody: ParsedTransactionBody | undefined
  readonly defaultExpanded?: boolean
  readonly title?: string
}

export function ParsedBodySection({
  parsedBody,
  contracts,
  formatAddress,
  onContractClick,
  defaultExpanded = false,
  title = "Parsed Body",
}: ParsedBodySectionProps) {
  const [isExpanded, setIsExpanded] = useState(defaultExpanded)
  const contentId = useId()

  if (!parsedBody) return null

  return (
    <section className={styles.parsedBodySection}>
      <div className={styles.parsedBodyTitle}>
        <span>{title}</span>
        <DisclosureToggle
          expanded={isExpanded}
          contextLabel={title.toLowerCase()}
          aria-controls={contentId}
          onClick={() => setIsExpanded(expanded => !expanded)}
        />
      </div>
      {isExpanded && (
        <div id={contentId} className={styles.parsedBodyTree}>
          <div className={styles.parsedBodyContent}>
            <ParsedValueView
              value={parsedBody.value}
              contracts={contracts}
              formatAddress={formatAddress}
              onContractClick={onContractClick}
              fallbackTypeName={parsedBody.name}
            />
          </div>
        </div>
      )}
    </section>
  )
}
