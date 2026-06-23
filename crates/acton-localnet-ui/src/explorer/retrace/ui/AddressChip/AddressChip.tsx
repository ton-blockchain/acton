import React from "react"

import {CopyButton} from "@retrace/CopyButton/CopyButton"

import styles from "./AddressChip.module.css"

export interface AddressChipProps {
  readonly address: string
  readonly className?: string
}

const AddressChip: React.FC<AddressChipProps> = ({address, className}) => {
  const chipClassName = `${styles.addressValue} ${className ?? ""}`.trim()

  return (
    <span className={chipClassName} title={address}>
      {address}
      <CopyButton className={styles.copyIcon} title="Copy address" value={address} />
    </span>
  )
}

export default AddressChip
