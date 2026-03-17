import type React from "react"

import type {ContractData} from "@/types/transaction"

import {ContractChip} from "../ContractChip/ContractChip"

import styles from "./StorageDiffView.module.css"
import type {StorageDiffNode, StorageDiffStatus, StorageLeafValue} from "./storageDiff"

interface StorageDiffViewProps {
  readonly diff: StorageDiffNode
  readonly contracts: Map<string, ContractData>
  readonly onContractClick?: (address: string) => void
}

const DECIMAL_SCALAR_PATTERN = /^-?\d+(?:\.\d+)?$/

function getEntryStatusClassName(status: StorageDiffStatus): string {
  switch (status) {
    case "added": {
      return styles.storageEntryAdded
    }
    case "removed": {
      return styles.storageEntryRemoved
    }
    case "changed": {
      return styles.storageEntryChanged
    }
    default: {
      return ""
    }
  }
}

function renderLeafValue(
  value: StorageLeafValue | undefined,
  contracts: Map<string, ContractData>,
  onContractClick?: (address: string) => void,
): React.JSX.Element {
  if (!value) {
    return <span className={styles.storageDiffPlaceholder}>—</span>
  }

  switch (value.kind) {
    case "null": {
      return <span className={styles.storageNullValue}>null</span>
    }
    case "address": {
      return (
        <ContractChip
          address={value.value}
          contracts={contracts}
          onContractClick={onContractClick}
        />
      )
    }
    case "boolean": {
      return (
        <span className={value.value ? styles.storageBooleanTrue : styles.storageBooleanFalse}>
          {value.value ? "true" : "false"}
        </span>
      )
    }
    case "scalar": {
      return (
        <span
          className={
            DECIMAL_SCALAR_PATTERN.test(value.value)
              ? styles.storagePlainValue
              : styles.storageLeafValue
          }
        >
          {value.value}
        </span>
      )
    }
  }
}

function getBeforePillClassName(status: StorageDiffStatus): string {
  switch (status) {
    case "added": {
      return styles.storageDiffPillNeutral
    }
    case "changed":
    case "removed": {
      return styles.storageDiffPillBefore
    }
    default: {
      return ""
    }
  }
}

function getAfterPillClassName(status: StorageDiffStatus): string {
  switch (status) {
    case "removed": {
      return styles.storageDiffPillNeutral
    }
    case "added":
    case "changed": {
      return styles.storageDiffPillAfter
    }
    default: {
      return ""
    }
  }
}

function StorageDiffRow({
  label,
  diff,
  contracts,
  onContractClick,
}: {
  readonly label: string
  readonly diff: StorageDiffNode
  readonly contracts: Map<string, ContractData>
  readonly onContractClick?: (address: string) => void
}): React.JSX.Element {
  const statusClassName = getEntryStatusClassName(diff.status)

  return (
    <>
      <div className={`${styles.storageEntryKey} ${statusClassName}`}>{label}:</div>
      <div className={`${styles.storageEntryValue} ${statusClassName}`}>
        <StorageDiffView
          diff={diff}
          contracts={contracts}
          onContractClick={onContractClick}
        />
      </div>
    </>
  )
}

export function StorageDiffView({
  diff,
  contracts,
  onContractClick,
}: StorageDiffViewProps): React.JSX.Element {
  if (diff.kind === "leaf") {
    if (diff.status === "unchanged") {
      return renderLeafValue(diff.after ?? diff.before, contracts, onContractClick)
    }

    return (
      <div className={styles.storageLeafDiff}>
        <span className={`${styles.storageDiffPill} ${getBeforePillClassName(diff.status)}`}>
          {renderLeafValue(diff.before, contracts, onContractClick)}
        </span>
        <span className={styles.storageDiffArrow}>→</span>
        <span className={`${styles.storageDiffPill} ${getAfterPillClassName(diff.status)}`}>
          {renderLeafValue(diff.after, contracts, onContractClick)}
        </span>
      </div>
    )
  }

  return (
    <div className={styles.storageObject}>
      {diff.typeName && <span className={styles.storageTypeLabel}>{diff.typeName}</span>}
      {diff.entries.length === 0 ? (
        <span className={styles.storageDiffPlaceholder}>—</span>
      ) : (
        <div className={styles.storageNested}>
          {diff.entries.map(entry => (
            <StorageDiffRow
              key={entry.key}
              label={entry.key}
              diff={entry.value}
              contracts={contracts}
              onContractClick={onContractClick}
            />
          ))}
        </div>
      )}
    </div>
  )
}
