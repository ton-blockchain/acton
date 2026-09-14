import type React from "react"
import {CopyInlineButton} from "@acton/ui"

import styles from "./TransactionDetails.module.css"

interface RawCopyAction {
  readonly value: string | undefined
  readonly label: string
  readonly caption: string
}

/** Keeps raw-data copy controls consistent across message and transaction detail sections. */
export function renderSectionCopyActions(
  actions: readonly RawCopyAction[],
): React.JSX.Element | undefined {
  const availableActions = actions.filter(
    (action): action is RawCopyAction & {readonly value: string} => action.value !== undefined,
  )
  if (availableActions.length === 0) {
    return undefined
  }

  return (
    <div className={styles.sectionCopyActions}>
      {availableActions.map(action => (
        <CopyInlineButton
          key={action.label}
          className={styles.sectionCopyButton}
          value={action.value}
          label={`Copy ${action.label}`}
          copiedLabel={`Copied ${action.label}`}
        >
          {action.caption}
        </CopyInlineButton>
      ))}
    </div>
  )
}
