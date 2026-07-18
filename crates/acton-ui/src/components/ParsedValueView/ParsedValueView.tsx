import {useState, type ReactNode} from "react"
import {Check, Copy, FileCode2} from "lucide-react"

import {ContractChip, type ContractReferenceOptions} from "../ContractChip/ContractChip"
import {CopyInlineAction, InlineAction, InlineActions} from "../InlineActions/InlineActions"
import {Popover} from "../Popover/Popover"
import {VisuallyGroupedNumber} from "../VisuallyGroupedNumber/VisuallyGroupedNumber"

import {
  formatScalarByFieldName,
  identifierHasWord,
  isDecimalScalarValue,
  isHexDisplayValue,
} from "./scalarDisplay"
import type {ParsedCodeCell, ParsedValue, ParsedValueMapEntry} from "./types"
import styles from "./ParsedValueView.module.css"

export interface ParsedValueViewProps extends ContractReferenceOptions {
  readonly value: ParsedValue
  readonly fallbackTypeName?: string
  readonly fieldName?: string
  readonly renderCodeCellDetails?: (cell: ParsedCodeCell) => ReactNode
}

type ParsedValueContext = ContractReferenceOptions &
  Pick<ParsedValueViewProps, "renderCodeCellDetails">

function ParsedTypeLabel({typeName}: {readonly typeName: string}) {
  return <span className={styles.parsedTypeLabel}>{typeName}</span>
}

function ParsedValueRow({
  label,
  value,
  contracts,
  formatAddress,
  onContractClick,
  renderCodeCellDetails,
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
          renderCodeCellDetails={renderCodeCellDetails}
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
  renderCodeCellDetails,
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
            renderCodeCellDetails={renderCodeCellDetails}
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
            renderCodeCellDetails={renderCodeCellDetails}
          />
        </div>
      </div>
    </div>
  )
}

function ParsedScalarValue({
  value,
  fieldName,
  renderCodeCellDetails,
}: {
  readonly value: Extract<ParsedValue, {readonly kind: "scalar"}>
  readonly fieldName?: string
  readonly renderCodeCellDetails?: (cell: ParsedCodeCell) => ReactNode
}) {
  const [isCodeOpen, setIsCodeOpen] = useState(false)
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
  const codeCell =
    value.typeName === "Cell" &&
    value.rawValue &&
    fieldName &&
    (identifierHasWord(fieldName, "code") || identifierHasWord(fieldName, "cell"))
      ? {bocHex: value.rawValue, fieldName}
      : undefined
  const canShowCode = codeCell !== undefined && renderCodeCellDetails !== undefined

  if (!value.rawValue && !canShowCode) return scalarValue

  return (
    <InlineActions
      visibility={isCodeOpen ? "always" : "hover"}
      actions={
        <>
          {canShowCode && (
            <Popover
              ariaLabel={`${fieldName} code`}
              interaction="click"
              placement="right"
              maxWidth="54rem"
              open={isCodeOpen}
              onOpenChange={setIsCodeOpen}
              triggerAsChild
              contentClassName={styles.codeCellPopover}
              content={isCodeOpen ? renderCodeCellDetails(codeCell) : null}
            >
              <InlineAction
                label={isCodeOpen ? "Hide code" : "Show code"}
                size="compact"
                icon={<FileCode2 />}
                aria-expanded={isCodeOpen}
              />
            </Popover>
          )}
          {value.rawValue && (
            <CopyInlineAction
              value={value.rawValue}
              label="Copy raw value"
              copiedLabel="Raw value copied"
              size="compact"
              icon={<Copy />}
              copiedIcon={<Check />}
            />
          )}
        </>
      }
    >
      {scalarValue}
    </InlineActions>
  )
}

export function ParsedValueView({
  value,
  contracts,
  formatAddress,
  onContractClick,
  fallbackTypeName,
  fieldName,
  renderCodeCellDetails,
}: ParsedValueViewProps) {
  const context = {contracts, formatAddress, onContractClick, renderCodeCellDetails}

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
      return (
        <ParsedScalarValue
          value={value}
          fieldName={fieldName}
          renderCodeCellDetails={renderCodeCellDetails}
        />
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
