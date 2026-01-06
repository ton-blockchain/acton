import type React from "react"
import type { ContractData } from "../../../types/transaction"
import styles from "./ContractChip.module.css"
import {formatAddress} from "../../../utils/format";

interface ContractChipProps {
  readonly address: string | undefined
  readonly contracts: Map<string, ContractData>
  readonly onContractClick?: (address: string) => void
}

export const ContractChip: React.FC<ContractChipProps> = ({
  address,
  contracts,
  onContractClick,
}) => {
  if (!address) {
    return <span className={styles.unknown}>Unknown</span>
  }

  const contract = contracts.get(address)
  const displayName = contract?.displayName ?? formatAddress(address)

  return (
    <button
      type="button"
      className={styles.chip}
      onClick={() => onContractClick?.(address)}
      title={address}
    >
      {contract?.letter && <span className={styles.letter}>{contract.letter}</span>}
      <span className={styles.name}>{displayName}</span>
    </button>
  )
}
