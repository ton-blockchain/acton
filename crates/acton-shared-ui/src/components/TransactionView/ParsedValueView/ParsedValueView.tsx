import type React from "react"

import type {ContractData, ParsedValue, ParsedValueMapEntry} from "@/types/transaction"
import {formatCurrency} from "@/utils/format"

import {CopyValueButton} from "../CopyValueButton"
import {ContractChip} from "../ContractChip/ContractChip"

import styles from "./ParsedValueView.module.css"

const DECIMAL_SCALAR_PATTERN = /^-?\d+(?:\.\d+)?$/
const INTEGER_SCALAR_PATTERN = /^-?\d+$/

type ParsedScalarValue = Extract<ParsedValue, {readonly kind: "scalar"}>

interface AddressFormatOptions {
  readonly testOnly?: boolean
}

function ParsedTypeLabel({typeName}: {readonly typeName: string}): React.JSX.Element {
  return <span className={styles.parsedTypeLabel}>{typeName}</span>
}

function ParsedValueRow({
  label,
  value,
  contracts,
  addressFormat,
  onContractClick,
}: {
  readonly label: string
  readonly value: ParsedValue
  readonly contracts: Map<string, ContractData>
  readonly addressFormat?: AddressFormatOptions
  readonly onContractClick?: (address: string) => void
}): React.JSX.Element {
  return (
    <>
      <div className={styles.parsedEntryKey}>{label}:</div>
      <div className={styles.parsedEntryValue}>
        <ParsedValueView
          value={value}
          contracts={contracts}
          addressFormat={addressFormat}
          onContractClick={onContractClick}
          fieldName={label}
        />
      </div>
    </>
  )
}

function ParsedMapEntry({
  entry,
  contracts,
  addressFormat,
  onContractClick,
}: {
  readonly entry: ParsedValueMapEntry
  readonly contracts: Map<string, ContractData>
  readonly addressFormat?: AddressFormatOptions
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
            addressFormat={addressFormat}
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
            addressFormat={addressFormat}
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
  readonly addressFormat?: AddressFormatOptions
  readonly onContractClick?: (address: string) => void
  readonly fallbackTypeName?: string
  readonly fieldName?: string
}

export function ParsedValueView({
  value,
  contracts,
  addressFormat,
  onContractClick,
  fallbackTypeName,
  fieldName,
}: ParsedValueViewProps): React.JSX.Element {
  switch (value.kind) {
    case "null": {
      return <span className={styles.parsedNull}>null</span>
    }
    case "void": {
      return <span className={styles.parsedVoid}>void</span>
    }
    case "address": {
      return (
        <ContractChip
          address={value.value}
          contracts={contracts}
          addressFormat={addressFormat}
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
      const displayValue = formatScalarByFieldName(value, fieldName)

      return (
        <span className={styles.scalarWithActions}>
          <span
            className={
              DECIMAL_SCALAR_PATTERN.test(value.value)
                ? styles.parsedPlainScalar
                : styles.parsedScalar
            }
          >
            {displayValue}
          </span>
          {value.rawValue && (
            <CopyValueButton className={styles.copyButton} value={value.rawValue} />
          )}
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
                addressFormat={addressFormat}
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
                  addressFormat={addressFormat}
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
          <span className={styles.parsedBadge}>{value.typeName ?? "map"}</span>
          {value.entries.length === 0 ? (
            <span className={styles.parsedEmpty}>{"{}"}</span>
          ) : (
            <div className={`${styles.parsedNested} ${styles.parsedNestedMap}`}>
              {value.entries.map((entry, index) => (
                <ParsedMapEntry
                  key={`map-entry-${index}`}
                  entry={entry}
                  contracts={contracts}
                  addressFormat={addressFormat}
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

function isAsciiAlphanumeric(value: string): boolean {
  return /^[A-Za-z0-9]$/.test(value)
}

function isAsciiDigit(value: string): boolean {
  return value >= "0" && value <= "9"
}

function isAsciiLowercase(value: string): boolean {
  return value >= "a" && value <= "z"
}

function isAsciiUppercase(value: string): boolean {
  return value >= "A" && value <= "Z"
}

function identifierWordBoundary(prev: string, current: string, next: string | undefined): boolean {
  if (isAsciiDigit(prev) !== isAsciiDigit(current)) {
    return true
  }

  if (isAsciiLowercase(prev) && isAsciiUppercase(current)) {
    return true
  }

  return (
    isAsciiUppercase(prev) &&
    isAsciiUppercase(current) &&
    next !== undefined &&
    isAsciiLowercase(next)
  )
}

function identifierHasWord(name: string, needle: string): boolean {
  let start: number | undefined
  let prev: string | undefined

  for (let index = 0; index < name.length; index += 1) {
    const current = name[index]
    if (!isAsciiAlphanumeric(current)) {
      if (start !== undefined && name.slice(start, index).toLowerCase() === needle.toLowerCase()) {
        return true
      }

      start = undefined
      prev = undefined
      continue
    }

    const next = index + 1 < name.length ? name[index + 1] : undefined
    if (prev !== undefined && start !== undefined && identifierWordBoundary(prev, current, next)) {
      if (name.slice(start, index).toLowerCase() === needle.toLowerCase()) {
        return true
      }
      start = index
    } else if (start === undefined) {
      start = index
    }

    prev = current
  }

  return start !== undefined && name.slice(start).toLowerCase() === needle.toLowerCase()
}

function formatScalarByFieldName(value: ParsedScalarValue, fieldName: string | undefined): string {
  if (
    value.typeName !== "coins" ||
    fieldName === undefined ||
    !identifierHasWord(fieldName, "ton") ||
    !INTEGER_SCALAR_PATTERN.test(value.value)
  ) {
    return value.value
  }

  try {
    return formatCurrency(BigInt(value.value))
  } catch {
    return value.value
  }
}
