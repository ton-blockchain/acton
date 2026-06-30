import type React from "react"

import {VisuallyGroupedNumber} from "@/components/VisuallyGroupedNumber/VisuallyGroupedNumber"
import type {ContractData} from "@/types/transaction"

import {ContractChip} from "../ContractChip/ContractChip"
import {CopyValueButton} from "../CopyValueButton"
import {formatScalarByFieldName, isDecimalScalarValue, isHexDisplayValue} from "../scalarDisplay"

import styles from "./StorageDiffView.module.css"
import type {StorageDiffNode, StorageDiffStatus, StorageLeafValue} from "./storageDiff"

export interface StorageDiffViewProps {
  readonly diff: StorageDiffNode
  readonly contracts: Map<string, ContractData>
  readonly onContractClick?: (address: string) => void
  readonly fieldName?: string
}

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
  fieldName?: string,
): React.JSX.Element {
  if (!value) {
    return <span className={styles.storageDiffPlaceholder}>—</span>
  }

  switch (value.kind) {
    case "null": {
      return <span className={styles.storageNullValue}>null</span>
    }
    case "void": {
      return <span className={styles.storageVoidValue}>void</span>
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
      const displayValue = formatScalarByFieldName({
        value: value.value,
        typeName: value.typeName,
        fieldName,
      })
      const valueElement = (
        <VisuallyGroupedNumber
          className={
            isDecimalScalarValue(value.value) && !isHexDisplayValue(displayValue)
              ? styles.storagePlainValue
              : styles.storageLeafValue
          }
          value={displayValue}
        />
      )

      if (value.rawValue) {
        return (
          <span className={styles.storageLeafWithActions}>
            {valueElement}
            <CopyValueButton className={styles.copyButton} value={value.rawValue} />
          </span>
        )
      }

      return valueElement
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
          fieldName={label}
        />
      </div>
    </>
  )
}

function StorageDiffMapRow({
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
    <div className={`${styles.storageMapEntry} ${statusClassName}`}>
      <div className={styles.storageMapSection}>
        <div className={styles.storageMapSectionLabel}>Key</div>
        <div className={styles.storageMapKey}>{label}</div>
      </div>
      <div className={styles.storageMapSection}>
        <div className={styles.storageMapSectionLabel}>Value</div>
        <div className={styles.storageMapValue}>
          <StorageDiffView diff={diff} contracts={contracts} onContractClick={onContractClick} />
        </div>
      </div>
    </div>
  )
}

export function StorageDiffView({
  diff,
  contracts,
  onContractClick,
  fieldName,
}: StorageDiffViewProps): React.JSX.Element {
  if (diff.kind === "leaf") {
    if (diff.status === "unchanged") {
      return renderLeafValue(diff.after ?? diff.before, contracts, onContractClick, fieldName)
    }

    return (
      <div className={styles.storageLeafDiff}>
        <span className={`${styles.storageDiffPill} ${getBeforePillClassName(diff.status)}`}>
          {renderLeafValue(diff.before, contracts, onContractClick, fieldName)}
        </span>
        <span className={styles.storageDiffArrow}>→</span>
        <span className={`${styles.storageDiffPill} ${getAfterPillClassName(diff.status)}`}>
          {renderLeafValue(diff.after, contracts, onContractClick, fieldName)}
        </span>
      </div>
    )
  }

  const showContainerLabel =
    diff.typeName && (diff.objectKind === "object" || diff.entries.length > 0)

  return (
    <div className={styles.storageObject}>
      {showContainerLabel && <span className={styles.storageTypeLabel}>{diff.typeName}</span>}
      {diff.entries.length === 0 ? (
        <span
          className={`${styles.storageDiffPill} ${styles.storageDiffPillNeutral} ${styles.storageEmptyPill}`}
        >
          <span className={styles.storageDiffPlaceholder}>—</span>
        </span>
      ) : diff.objectKind === "map" ? (
        <div className={styles.storageNestedMap}>
          {diff.entries.map(entry => (
            <StorageDiffMapRow
              key={entry.key}
              label={entry.key}
              diff={entry.value}
              contracts={contracts}
              onContractClick={onContractClick}
            />
          ))}
        </div>
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
