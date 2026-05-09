import * as React from "react"
import {useEffect, useState} from "react"

import type {ContractData, ParsedValue, ParsedValueMapEntry} from "@/types/transaction"

import {ContractChip} from "../ContractChip/ContractChip"

import styles from "./ParsedValueView.module.css"

const DECIMAL_SCALAR_PATTERN = /^-?\d+(?:\.\d+)?$/

function ParsedTypeLabel({typeName}: {readonly typeName: string}): React.JSX.Element {
  return <span className={styles.parsedTypeLabel}>{typeName}</span>
}

function CopyValueButton({rawValue}: {readonly rawValue: string}): React.JSX.Element {
  const [isCopied, setIsCopied] = useState(false)

  const handleCopy = (): void => {
    navigator.clipboard
      .writeText(rawValue)
      .then(() => {
        setIsCopied(true)
      })
      .catch((error: unknown) => {
        console.error("Failed to copy:", error)
      })
  }

  useEffect(() => {
    if (!isCopied) {
      return
    }

    const timer = setTimeout(() => setIsCopied(false), 2000)
    return () => clearTimeout(timer)
  }, [isCopied])

  return (
    <button
      type="button"
      className={styles.copyButton}
      onClick={event => {
        event.stopPropagation()
        handleCopy()
      }}
      title={isCopied ? "Copied full BOC hex" : "Copy full BOC hex"}
      aria-label={isCopied ? "Copied full BOC hex" : "Copy full BOC hex"}
    >
      {isCopied ? (
        <svg
          width="12"
          height="12"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          strokeWidth="2"
        >
          <title>Copied</title>
          <polyline points="20,6 9,17 4,12" />
        </svg>
      ) : (
        <svg
          width="12"
          height="12"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          strokeWidth="2"
        >
          <title>Copy full BOC hex</title>
          <rect x="9" y="9" width="13" height="13" rx="2" ry="2" />
          <path d="m5,15 L5,5 a2,2 0 0,1 2,-2 l10,0" />
        </svg>
      )}
    </button>
  )
}

function ParsedValueRow({
  label,
  value,
  contracts,
  onContractClick,
}: {
  readonly label: string
  readonly value: ParsedValue
  readonly contracts: Map<string, ContractData>
  readonly onContractClick?: (address: string) => void
}): React.JSX.Element {
  return (
    <>
      <div className={styles.parsedEntryKey}>{label}:</div>
      <div className={styles.parsedEntryValue}>
        <ParsedValueView value={value} contracts={contracts} onContractClick={onContractClick} />
      </div>
    </>
  )
}

function ParsedMapEntry({
  entry,
  contracts,
  onContractClick,
}: {
  readonly entry: ParsedValueMapEntry
  readonly contracts: Map<string, ContractData>
  readonly onContractClick?: (address: string) => void
}): React.JSX.Element {
  return (
    <div className={styles.parsedMapEntry}>
      <div className={styles.parsedMapSection}>
        <div className={styles.parsedMapSectionLabel}>Key</div>
        <div className={styles.parsedMapSectionValue}>
          <ParsedValueView
            value={entry.key}
            contracts={contracts}
            onContractClick={onContractClick}
          />
        </div>
      </div>
      <div className={styles.parsedMapSection}>
        <div className={styles.parsedMapSectionLabel}>Value</div>
        <div className={styles.parsedMapSectionValue}>
          <ParsedValueView
            value={entry.value}
            contracts={contracts}
            onContractClick={onContractClick}
          />
        </div>
      </div>
    </div>
  )
}

interface ParsedValueViewProps {
  readonly value: ParsedValue
  readonly contracts: Map<string, ContractData>
  readonly onContractClick?: (address: string) => void
  readonly fallbackTypeName?: string
}

export function ParsedValueView({
  value,
  contracts,
  onContractClick,
  fallbackTypeName,
}: ParsedValueViewProps): React.JSX.Element {
  switch (value.kind) {
    case "null": {
      return <span className={styles.parsedNull}>null</span>
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
        <span className={value.value ? styles.booleanTrue : styles.booleanFalse}>
          {value.value ? "true" : "false"}
        </span>
      )
    }
    case "scalar": {
      return (
        <span className={styles.scalarWithActions}>
          <span
            className={
              DECIMAL_SCALAR_PATTERN.test(value.value)
                ? styles.parsedPlainScalar
                : styles.parsedScalar
            }
          >
            {value.value}
          </span>
          {value.rawValue && <CopyValueButton rawValue={value.rawValue} />}
        </span>
      )
    }
    case "array": {
      if (value.items.length === 0) {
        return <span className={styles.parsedEmpty}>[]</span>
      }

      return (
        <div className={styles.parsedContainer}>
          <span className={styles.parsedBadge}>array</span>
          <div className={styles.parsedNested}>
            {value.items.map((item, index) => (
              <ParsedValueRow
                key={`array-item-${index}`}
                label={`[${index}]`}
                value={item}
                contracts={contracts}
                onContractClick={onContractClick}
              />
            ))}
          </div>
        </div>
      )
    }
    case "object": {
      const typeName = value.typeName ?? fallbackTypeName

      return (
        <div className={styles.parsedContainer}>
          {typeName && <ParsedTypeLabel typeName={typeName} />}
          {value.entries.length === 0 ? (
            <span className={styles.parsedEmpty}>{"{}"}</span>
          ) : (
            <div className={styles.parsedNested}>
              {value.entries.map(entry => (
                <ParsedValueRow
                  key={entry.key}
                  label={entry.key}
                  value={entry.value}
                  contracts={contracts}
                  onContractClick={onContractClick}
                />
              ))}
            </div>
          )}
        </div>
      )
    }
    case "map": {
      return (
        <div className={styles.parsedContainer}>
          <span className={styles.parsedBadge}>map</span>
          {value.entries.length === 0 ? (
            <span className={styles.parsedEmpty}>{"{}"}</span>
          ) : (
            <div className={`${styles.parsedNested} ${styles.parsedNestedMap}`}>
              {value.entries.map((entry, index) => (
                <ParsedMapEntry
                  key={`map-entry-${index}`}
                  entry={entry}
                  contracts={contracts}
                  onContractClick={onContractClick}
                />
              ))}
            </div>
          )}
        </div>
      )
    }
  }
}
