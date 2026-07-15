import {Check, Copy} from "lucide-react"

import {ContractChip, type ContractReferenceOptions} from "../ContractChip/ContractChip"
import {CopyInlineAction, InlineActions} from "../InlineActions/InlineActions"
import {VisuallyGroupedNumber} from "../VisuallyGroupedNumber/VisuallyGroupedNumber"

import {formatScalarByFieldName, isDecimalScalarValue, isHexDisplayValue} from "./scalarDisplay"
import type {ParsedValue, ParsedValueMapEntry} from "./types"
import styles from "./ParsedValueView.module.css"

export interface ParsedValueViewProps extends ContractReferenceOptions {
  readonly value: ParsedValue
  readonly fallbackTypeName?: string
  readonly fieldName?: string
}

type ParsedValueContext = ContractReferenceOptions

function ParsedTypeLabel({typeName}: {readonly typeName: string}) {
  return <span className={styles.parsedTypeLabel}>{typeName}</span>
}

function ParsedValueRow({
  label,
  value,
  contracts,
  formatAddress,
  onContractClick,
}: ParsedValueContext & {readonly label: string; readonly value: ParsedValue}) {
  return (
    <>
      <div className={styles.parsedEntryKey}>{label}:</div>
      <div className={styles.parsedEntryValue}>
        <ParsedValueView
          value={value}
          contracts={contracts}
          formatAddress={formatAddress}
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
  formatAddress,
  onContractClick,
}: ParsedValueContext & {readonly entry: ParsedValueMapEntry}) {
  return (
    <div className={styles.parsedMapEntry}>
      <div className={styles.parsedMapSection}>
        <div className={styles.parsedMapSectionLabel}>Key</div>
        <div className={styles.parsedMapSectionValue}>
          <ParsedValueView
            value={entry.key}
            contracts={contracts}
            formatAddress={formatAddress}
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
            formatAddress={formatAddress}
            onContractClick={onContractClick}
          />
        </div>
      </div>
    </div>
  )
}

export function ParsedValueView({
  value,
  contracts,
  formatAddress,
  onContractClick,
  fallbackTypeName,
  fieldName,
}: ParsedValueViewProps) {
  const context = {contracts, formatAddress, onContractClick}

  switch (value.kind) {
    case "null":
      return <span className={styles.parsedNull}>null</span>
    case "void":
      return <span className={styles.parsedVoid}>void</span>
    case "address":
      return <ContractChip address={value.value} {...context} />
    case "boolean":
      return (
        <span className={value.value ? styles.booleanTrue : styles.booleanFalse}>
          {value.value ? "true" : "false"}
        </span>
      )
    case "scalar": {
      const displayValue = formatScalarByFieldName({
        value: value.value,
        typeName: value.typeName,
        fieldName,
      })
      const scalarValue = (
        <VisuallyGroupedNumber
          className={
            isDecimalScalarValue(value.value) && !isHexDisplayValue(displayValue)
              ? styles.parsedPlainScalar
              : styles.parsedScalar
          }
          value={displayValue}
        />
      )

      if (!value.rawValue) return scalarValue

      return (
        <InlineActions
          actions={
            <CopyInlineAction
              value={value.rawValue}
              label="Copy raw value"
              copiedLabel="Raw value copied"
              size="compact"
              icon={<Copy />}
              copiedIcon={<Check />}
            />
          }
        >
          {scalarValue}
        </InlineActions>
      )
    }
    case "array":
      if (value.items.length === 0) return <span className={styles.parsedEmpty}>[]</span>

      return (
        <div className={styles.parsedContainer}>
          <span className={styles.parsedBadge}>array</span>
          <div className={styles.parsedNested}>
            {value.items.map((item, index) => (
              <ParsedValueRow
                // react-doctor-disable-next-line react-doctor/no-array-index-as-key -- the index is the semantic identity of an array item
                key={`array-item-${index}`}
                label={`[${index}]`}
                value={item}
                {...context}
              />
            ))}
          </div>
        </div>
      )
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
                  {...context}
                />
              ))}
            </div>
          )}
        </div>
      )
    }
    case "map":
      return (
        <div className={styles.parsedContainer}>
          <span className={styles.parsedBadge}>{value.typeName ?? "map"}</span>
          {value.entries.length === 0 ? (
            <span className={styles.parsedEmpty}>{"{}"}</span>
          ) : (
            <div className={`${styles.parsedNested} ${styles.parsedNestedMap}`}>
              {value.entries.map(entry => (
                <ParsedMapEntry key={JSON.stringify(entry.key)} entry={entry} {...context} />
              ))}
            </div>
          )}
        </div>
      )
  }
}
