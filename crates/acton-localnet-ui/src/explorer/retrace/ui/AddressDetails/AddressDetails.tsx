import React from "react"

import {Address} from "@ton/core"

import AddressChip from "@retrace/ui/AddressChip"

import styles from "./AddressDetails.module.css"

interface AddressDetailsProps {
  readonly address: Address
}

const AddressDetails: React.FC<AddressDetailsProps> = ({address}) => {
  const rawString = address.toRawString()
  const readableString = address.toString()

  return (
    <div className={styles.addressSection}>
      <div className={styles.addressRow}>
        <div className={styles.addressLabel}>Raw Address:</div>
        <div className={styles.addressValue}>
          <AddressChip address={rawString} />
        </div>
      </div>
      <div className={styles.addressRow}>
        <div className={styles.addressLabel}>Friendly Address:</div>
        <div className={styles.addressValue}>
          <AddressChip address={readableString} />
        </div>
      </div>
    </div>
  )
}

export default AddressDetails
