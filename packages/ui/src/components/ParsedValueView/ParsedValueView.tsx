import {useId, useState, type ReactNode} from "react"
import {Binary, Check, Copy, FileCode2} from "lucide-react"

import {ContractChip, type ContractReferenceOptions} from "../ContractChip/ContractChip"
import {CountValue} from "../CountValue/CountValue"
import {DisclosureToggle} from "../DisclosureToggle/DisclosureToggle"
import {InfoPopover} from "../InfoPopover/InfoPopover"
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
  readonly onCellInspect?: (boc: string) => void
  readonly renderCodeCellDetails?: (cell: ParsedCodeCell) => ReactNode
}

type ParsedValueContext = ContractReferenceOptions &
  Pick<ParsedValueViewProps, "onCellInspect" | "renderCodeCellDetails">

const LARGE_COLLECTION_THRESHOLD = 8

function ParsedCollection({
  label,
  itemCount,
  contentClassName,
  children,
}: {
  readonly label: string
  readonly itemCount: number
  readonly contentClassName?: string
  readonly children: ReactNode
}) {
  const [isExpanded, setIsExpanded] = useState(false)
  const contentId = useId()
  const isLarge = itemCount > LARGE_COLLECTION_THRESHOLD
  const content = (
    <div
      id={isLarge ? contentId : undefined}
      className={`${styles.parsedNested} ${contentClassName ?? ""} ${isLarge ? styles.parsedCollectionViewport : ""}`}
    >
      {children}
    </div>
  )

  if (!isLarge) {
    return (
      <>
        <span className={styles.parsedBadge}>{label}</span>
        {content}
      </>
    )
  }

  return (
    <>
      <div className={styles.parsedCollectionHeader}>
        <span className={styles.parsedBadge}>{label}</span>
        <span className={styles.parsedCollectionCount}>
          <CountValue singular="item" value={itemCount} />
        </span>
        <DisclosureToggle
          className={styles.parsedCollectionToggle}
          expanded={isExpanded}
          showLabel="Expand"
          hideLabel="Collapse"
          contextLabel={`${label} collection`}
          aria-controls={contentId}
          onClick={() => setIsExpanded(expanded => !expanded)}
        />
      </div>
      {isExpanded && content}
    </>
  )
}

function ParsedTypeLabel({typeName}: {readonly typeName: string}) {
  return <span className={styles.parsedTypeLabel}>{typeName}</span>
}

function ParsedValueRow({
  label,
  value,
  contracts,
  formatAddress,
  onContractClick,
  onCellInspect,
  renderCodeCellDetails,
}: ParsedValueContext & {readonly label: string; readonly value: ParsedValue}) {
  const isLargeCollection =
    (value.kind === "array" && value.items.length > LARGE_COLLECTION_THRESHOLD) ||
    (value.kind === "map" && value.entries.length > LARGE_COLLECTION_THRESHOLD)

  return (
    <>
      <div className={styles.parsedEntryKey}>{label}:</div>
      <div
        className={`${styles.parsedEntryValue} ${isLargeCollection ? styles.parsedLargeCollectionValue : ""}`}
      >
        <ParsedValueView
          value={value}
          contracts={contracts}
          formatAddress={formatAddress}
          onContractClick={onContractClick}
          onCellInspect={onCellInspect}
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
  onCellInspect,
  renderCodeCellDetails,
}: ParsedValueContext & {readonly entry: ParsedValueMapEntry}) {
  return (
    <div className={styles.parsedMapEntry}>
      <div className={styles.parsedMapSection}>
        <div className={styles.parsedMapSectionLabel}>Key</div>
        <div
          className={
            entry.keyInfo
              ? `${styles.parsedMapSectionValue} ${styles.parsedMapKeyValue}`
              : styles.parsedMapSectionValue
          }
        >
          <ParsedValueView
            value={entry.key}
            contracts={contracts}
            formatAddress={formatAddress}
            onContractClick={onContractClick}
            onCellInspect={onCellInspect}
            renderCodeCellDetails={renderCodeCellDetails}
            fieldName={
              entry.key.kind === "scalar" && entry.key.typeName === "uint256" ? "key" : undefined
            }
          />
          {entry.keyInfo && (
            <InfoPopover ariaLabel={`About parameter ID ${entry.keyInfo.id}`}>
              <div className={styles.parsedMapKeyInfo}>
                <span className={styles.parsedMapKeyInfoId}>Parameter ID: {entry.keyInfo.id}</span>
                {entry.keyInfo.description && (
                  <span className={styles.parsedMapKeyInfoDescription}>
                    {entry.keyInfo.description}
                  </span>
                )}
              </div>
            </InfoPopover>
          )}
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
            onCellInspect={onCellInspect}
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
  onCellInspect,
  renderCodeCellDetails,
}: {
  readonly value: Extract<ParsedValue, {readonly kind: "scalar"}>
  readonly fieldName?: string
  readonly onCellInspect?: (boc: string) => void
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
  const inspectableType =
    value.typeName === "Cell" || value.typeName === "Slice" ? value.typeName : undefined
  const inspectableBoc = inspectableType ? value.rawValue : undefined
  const canInspectCell = inspectableBoc !== undefined && onCellInspect !== undefined
  const inspectableLabel = inspectableType?.toLowerCase()

  if (!value.rawValue && !canShowCode && !canInspectCell) return scalarValue

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
                title={
                  isCodeOpen ? "Hide decoded code for this cell" : "Show decoded code for this cell"
                }
                size="compact"
                icon={<FileCode2 />}
                aria-expanded={isCodeOpen}
              />
            </Popover>
          )}
          {canInspectCell && (
            <InlineAction
              label={`Inspect ${inspectableLabel}`}
              title={`Open this ${inspectableLabel} in the Cell Inspector`}
              size="compact"
              icon={<Binary />}
              onClick={() => onCellInspect(inspectableBoc)}
            />
          )}
          {value.rawValue && (
            <CopyInlineAction
              value={value.rawValue}
              label="Copy raw value"
              copiedLabel="Raw value copied"
              title={`Copy this ${inspectableLabel ?? "value"}'s raw BoC as hex`}
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
  onCellInspect,
  renderCodeCellDetails,
}: ParsedValueViewProps) {
  const context = {contracts, formatAddress, onContractClick, onCellInspect, renderCodeCellDetails}

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
          onCellInspect={onCellInspect}
          renderCodeCellDetails={renderCodeCellDetails}
        />
      )
    }
    case "array":
      if (value.items.length === 0) return <span className={styles.parsedEmpty}>[]</span>

      return (
        <div className={styles.parsedContainer}>
          <ParsedCollection label="array" itemCount={value.items.length}>
            {value.items.map((item, index) => (
              <ParsedValueRow
                key={`array-item-${index}`}
                label={`[${index}]`}
                value={item}
                {...context}
              />
            ))}
          </ParsedCollection>
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
          {value.entries.length === 0 ? (
            <>
              <span className={styles.parsedBadge}>{value.typeName ?? "map"}</span>
              <span className={styles.parsedEmpty}>{"{}"}</span>
            </>
          ) : (
            <ParsedCollection
              label={value.typeName ?? "map"}
              itemCount={value.entries.length}
              contentClassName={styles.parsedNestedMap}
            >
              {value.entries.map(entry => (
                <ParsedMapEntry key={JSON.stringify(entry.key)} entry={entry} {...context} />
              ))}
            </ParsedCollection>
          )}
        </div>
      )
  }
}
