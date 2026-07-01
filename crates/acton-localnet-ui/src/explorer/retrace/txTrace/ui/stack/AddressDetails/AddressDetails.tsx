import {ContractChip, type ContractData} from "@acton/shared-ui"

import {Address} from "@ton/core"
import React, {useMemo} from "react"

import {AddressChip} from "../../../../../components/AddressChip"
import type {ExplorerNavigationClickEvent} from "../../../../../hooks/useOpenExplorerPath"

import styles from "./AddressDetails.module.css"

interface AddressDetailsProps {
  readonly address: Address
  readonly contracts?: Map<string, ContractData>
  readonly onContractClick?: (address: string, event?: ExplorerNavigationClickEvent) => void
}

const EMPTY_CONTRACTS = new Map<string, ContractData>()

const AddressValueRow: React.FC<{
  readonly label: string
  readonly value: string
  readonly displayFormat?: "network" | "raw"
  readonly onAddressClick?: (address: string, event?: ExplorerNavigationClickEvent) => void
}> = ({label, value, displayFormat = "network", onAddressClick}) => (
  <div className={styles.addressRow}>
    <div className={styles.addressLabel}>{label}</div>
    <div className={styles.addressChipLine}>
      <AddressChip
        address={value}
        copyPlacement="right"
        displayFormat={displayFormat}
        onAddressClick={onAddressClick}
        resolveName={false}
        shorten={false}
      />
    </div>
  </div>
)

const AddressDetails: React.FC<AddressDetailsProps> = ({
  address,
  contracts = EMPTY_CONTRACTS,
  onContractClick,
}) => {
  const rawString = address.toRawString()
  const readableString = address.toString()
  const contract = contracts.get(readableString) ?? contracts.get(rawString)
  const addressContracts = useMemo(() => {
    if (!contract) {
      return contracts
    }

    const nextContracts = new Map(contracts)
    nextContracts.set(rawString, contract)
    nextContracts.set(readableString, contract)
    return nextContracts
  }, [contract, contracts, rawString, readableString])

  return (
    <div className={styles.addressSection}>
      {contract && (
        <div className={styles.contractLine}>
          <ContractChip
            address={readableString}
            className={styles.contractChip}
            contracts={addressContracts}
            onContractClick={onContractClick}
          />
        </div>
      )}
      <div className={styles.rawAddressRow}>
        <AddressValueRow
          label="Raw"
          value={rawString}
          displayFormat="raw"
          onAddressClick={onContractClick}
        />
      </div>
    </div>
  )
}

export default AddressDetails
