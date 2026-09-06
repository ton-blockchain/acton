import {RawDataBlock} from "@acton/ui"

import styles from "./ConfigEditor.module.css"

/** Keeps TL-B diagnostics intact while separating the failed expression from prose */
export function ParameterErrorDescription({error}: {readonly error: unknown}) {
  const message = error instanceof Error ? error.message : String(error)
  const condition = /^Condition \((.+)\) is not satisfied(.*)$/s.exec(message)

  if (!condition) return message

  const context = condition[2].trim()

  return (
    <span className={styles.errorDescription}>
      <span>Required condition is not met</span>
      <RawDataBlock
        value={condition[1]}
        showCopy={false}
        contentClassName={styles.conditionContent}
      />
      {context && <span>{context.charAt(0).toUpperCase() + context.slice(1)}</span>}
    </span>
  )
}
