import React from "react"

import {Address} from "@ton/core"

import {ContractChip, type ContractData} from "@acton/shared-ui"

import styles from "./AddressDetails.module.css"

interface AddressDetailsProps {
  readonly address: Address
}

const EMPTY_CONTRACTS = new Map<string, ContractData>()

const AddressDetails: React.FC<AddressDetailsProps> = ({address}) => {
  const rawString = address.toRawString()
  const readableString = address.toString()

  return (
    <div className={styles.addressSection}>
      <div className={styles.addressRow}>
        <div className={styles.addressLabel}>Raw Address:</div>
        <div className={styles.addressValue}>
          <ContractChip
            address={rawString}
            contracts={EMPTY_CONTRACTS}
            trimSoloAddress={false}
          />
        </div>
      </div>
      <div className={styles.addressRow}>
        <div className={styles.addressLabel}>Friendly Address:</div>
        <div className={styles.addressValue}>
          <ContractChip
            address={readableString}
            contracts={EMPTY_CONTRACTS}
            trimSoloAddress={false}
          />
        </div>
      </div>
    </div>
  )
}

export default AddressDetails
