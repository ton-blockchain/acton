import type {LocalnetContractStatus} from "@acton/explorer-core/api/types"

import {contractStatusLabels} from "./contractPresentation"
import styles from "./ContractStatus.module.css"

export function ContractStatus({status}: {readonly status: LocalnetContractStatus}) {
  return (
    <span className={styles.status} data-status={status}>
      <span className={styles.dot} aria-hidden="true" />
      {contractStatusLabels[status]}
    </span>
  )
}
