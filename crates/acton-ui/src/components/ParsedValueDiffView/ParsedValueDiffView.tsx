import type React from "react"
import {ArrowRight} from "lucide-react"

import type {ContractReferenceOptions} from "../ContractChip/ContractChip"
import {ParsedValueView} from "../ParsedValueView/ParsedValueView"
import type {ParsedValueLeaf} from "../ParsedValueView/types"

import styles from "./ParsedValueDiffView.module.css"
import type {ParsedValueDiff, ParsedValueDiffStatus} from "./types"

export interface ParsedValueDiffViewProps extends ContractReferenceOptions {
  readonly diff: ParsedValueDiff
  readonly fieldName?: string
}

type ParsedValueDiffContext = ContractReferenceOptions

function getEntryStatusClassName(status: ParsedValueDiffStatus): string | undefined {
  switch (status) {
    case "added":
      return styles.entryAdded
    case "removed":
      return styles.entryRemoved
    case "changed":
      return styles.entryChanged
    default:
      return undefined
  }
}

function renderLeafValue(
  value: ParsedValueLeaf | undefined,
  context: ParsedValueDiffContext,
  fieldName?: string,
): React.JSX.Element {
  if (!value) return <span className={styles.diffPlaceholder}>—</span>

  return <ParsedValueView value={value} fieldName={fieldName} {...context} />
}

function getBeforePillClassName(status: ParsedValueDiffStatus): string | undefined {
  switch (status) {
    case "added":
      return styles.diffPillNeutral
    case "changed":
    case "removed":
      return styles.diffPillBefore
    default:
      return undefined
  }
}

function getAfterPillClassName(status: ParsedValueDiffStatus): string | undefined {
  switch (status) {
    case "removed":
      return styles.diffPillNeutral
    case "added":
    case "changed":
      return styles.diffPillAfter
    default:
      return undefined
  }
}

function DiffArrow(): React.JSX.Element {
  return (
    <span className={styles.diffArrow} aria-hidden="true">
      <ArrowRight />
    </span>
  )
}

function renderEntryValue(
  diff: ParsedValueDiff,
  context: ParsedValueDiffContext,
  fieldName?: string,
): React.JSX.Element {
  if (diff.kind === "leaf" && (diff.status === "added" || diff.status === "removed")) {
    return (
      <span className={styles.leafValue}>
        {renderLeafValue(diff.status === "added" ? diff.after : diff.before, context, fieldName)}
      </span>
    )
  }

  return <ParsedValueDiffView diff={diff} fieldName={fieldName} {...context} />
}

function renderArrayEntryValue(
  diff: ParsedValueDiff,
  context: ParsedValueDiffContext,
  fieldName: string,
): React.JSX.Element {
  if (diff.kind === "leaf" && (diff.status === "added" || diff.status === "removed")) {
    const value = diff.status === "added" ? diff.after : diff.before
    const pillClassName = diff.status === "added" ? styles.diffPillAfter : styles.diffPillBefore

    return (
      <span className={`${styles.diffPill} ${pillClassName}`}>
        {renderLeafValue(value, context, fieldName)}
      </span>
    )
  }

  return <ParsedValueDiffView diff={diff} fieldName={fieldName} {...context} />
}

function ParsedValueDiffRow({
  label,
  diff,
  compactEntryChange,
  ...context
}: ParsedValueDiffContext & {
  readonly label: string
  readonly diff: ParsedValueDiff
  readonly compactEntryChange: boolean
}): React.JSX.Element {
  const statusClassName = getEntryStatusClassName(diff.status)

  return (
    <div className={styles.nestedEntry}>
      <div className={`${styles.entryKey} ${statusClassName ?? ""}`}>{label}:</div>
      <div className={`${styles.entryValue} ${statusClassName ?? ""}`}>
        {compactEntryChange ? (
          renderArrayEntryValue(diff, context, label)
        ) : (
          <ParsedValueDiffView diff={diff} fieldName={label} {...context} />
        )}
      </div>
    </div>
  )
}

function ParsedValueDiffMapRow({
  label,
  diff,
  ...context
}: ParsedValueDiffContext & {
  readonly label: string
  readonly diff: ParsedValueDiff
}): React.JSX.Element {
  const statusClassName = getEntryStatusClassName(diff.status)

  return (
    <div className={`${styles.mapEntry} ${statusClassName ?? ""}`}>
      <div className={styles.mapSection}>
        <div className={styles.mapSectionLabel}>Key</div>
        <div className={styles.mapKey}>{label}</div>
      </div>
      <div className={styles.mapSection}>
        <div className={styles.mapSectionLabel}>Value</div>
        <div className={styles.mapValue}>{renderEntryValue(diff, context)}</div>
      </div>
    </div>
  )
}

export function ParsedValueDiffView({
  diff,
  fieldName,
  contracts,
  formatAddress,
  onContractClick,
}: ParsedValueDiffViewProps): React.JSX.Element {
  const context = {contracts, formatAddress, onContractClick}

  if (diff.kind === "leaf") {
    if (diff.status === "unchanged") {
      return (
        <span className={styles.leafValue}>
          {renderLeafValue(diff.after ?? diff.before, context, fieldName)}
        </span>
      )
    }

    return (
      <div className={styles.leafDiff}>
        <span className={`${styles.diffPill} ${getBeforePillClassName(diff.status) ?? ""}`}>
          {renderLeafValue(diff.before, context, fieldName)}
        </span>
        <DiffArrow />
        <span className={`${styles.diffPill} ${getAfterPillClassName(diff.status) ?? ""}`}>
          {renderLeafValue(diff.after, context, fieldName)}
        </span>
      </div>
    )
  }

  const statusClassName = getEntryStatusClassName(diff.status)
  const emptyValue = diff.objectKind === "array" ? "[]" : "{}"

  return (
    <div className={styles.root}>
      {diff.typeName && <span className={styles.typeLabel}>{diff.typeName}</span>}
      {diff.entries.length === 0 ? (
        <span className={`${styles.emptyValue} ${statusClassName ?? ""}`}>{emptyValue}</span>
      ) : diff.objectKind === "map" ? (
        <div className={styles.nestedMap}>
          {diff.entries.map(entry => (
            <ParsedValueDiffMapRow
              key={entry.key}
              label={entry.key}
              diff={entry.value}
              {...context}
            />
          ))}
        </div>
      ) : (
        <div className={styles.nested}>
          {diff.entries.map(entry => (
            <ParsedValueDiffRow
              key={entry.key}
              label={entry.key}
              diff={entry.value}
              compactEntryChange={diff.objectKind === "array"}
              {...context}
            />
          ))}
        </div>
      )}
    </div>
  )
}
