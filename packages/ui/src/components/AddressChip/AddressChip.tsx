import {Check, Copy} from "lucide-react"
import type {MouseEvent, ReactNode} from "react"

import {cx} from "../../lib/cx"
import {shortenMiddle} from "../../lib/formatting"
import {useCopyValue} from "../../lib/useCopyValue"
import {CopyInlineAction, InlineAction} from "../InlineActions/InlineActions"
import {Tooltip} from "../Tooltip"

import styles from "./AddressChip.module.css"

export type AddressChipCopyPlacement = "left" | "right"
export type AddressChipVariant = "accent" | "plain"
export type AddressChipAddressFormatter = (address: string) => string

export interface AddressChipTooltipVariant {
  readonly label: string
  readonly value: string
}

export interface AddressChipProps {
  readonly address: string | undefined
  readonly className?: string
  readonly copied?: boolean
  readonly copyable?: boolean
  readonly copyPlacement?: AddressChipCopyPlacement
  readonly copyResetDelay?: number
  readonly fallback?: ReactNode
  readonly formatAddress?: AddressChipAddressFormatter
  readonly highlighted?: boolean
  readonly label?: ReactNode
  readonly onAddressClick?: (address: string, event?: MouseEvent<HTMLElement>) => void
  readonly onCopyAddress?: (address: string) => Promise<void> | void
  readonly onCopyError?: (error: unknown) => void
  readonly onHoverAddressChange?: (address: string | undefined) => void
  readonly shorten?: boolean
  readonly tooltipVariants?: readonly AddressChipTooltipVariant[]
  readonly variant?: AddressChipVariant
}

export function AddressChip({
  address,
  className,
  copied = false,
  copyable = true,
  copyPlacement = "right",
  copyResetDelay = 1600,
  fallback = "Unknown",
  formatAddress,
  highlighted = false,
  label,
  onAddressClick,
  onCopyAddress,
  onCopyError,
  onHoverAddressChange,
  shorten = true,
  tooltipVariants,
  variant = "accent",
}: AddressChipProps) {
  const displayAddress = address ? getDisplayAddress(address, formatAddress) : ""
  const {copy, isCopied: isCopiedInternally} = useCopyValue({
    value: displayAddress,
    onCopy: address && onCopyAddress ? () => onCopyAddress(address) : undefined,
    onCopyError,
    resetDelay: copyResetDelay,
  })
  const isCopied = copied || isCopiedInternally
  const addressContent = address
    ? shorten
      ? shortenAddress(displayAddress)
      : displayAddress
    : fallback
  const isClickable = address !== undefined && onAddressClick !== undefined
  const addressClassName = cx(
    isClickable ? styles.addressButton : styles.addressText,
    highlighted && styles.addressHighlighted,
    !shorten && styles.addressFull,
    variant === "plain" && styles.addressPlain,
  )

  const addressNode =
    address !== undefined && onAddressClick !== undefined ? (
      <button
        type="button"
        className={addressClassName}
        onClick={event => {
          event.stopPropagation()
          onAddressClick(address, event)
        }}
      >
        {label ?? addressContent}
      </button>
    ) : (
      <span className={addressClassName}>{label ?? addressContent}</span>
    )

  if (!address) return addressNode

  const addressWithTooltip = (
    <Tooltip
      content={
        <AddressChipTooltip
          address={address}
          displayAddress={displayAddress}
          onCopyError={onCopyError}
          tooltipVariants={tooltipVariants}
        />
      }
      width="extra-wide"
    >
      {addressNode}
    </Tooltip>
  )

  const copyButton = copyable ? (
    <InlineAction
      className={cx(styles.copyButton, isCopied && styles.copyButtonCopied)}
      label={isCopied ? "Address copied" : "Copy address"}
      icon={isCopied ? <Check /> : <Copy />}
      onClick={event => {
        event.stopPropagation()
        void copy()
      }}
    />
  ) : null

  return (
    // biome-ignore lint/a11y/noStaticElementInteractions: Hover handlers only synchronize optional highlighting and do not perform an action.
    <span
      className={cx(
        styles.addressCluster,
        !shorten && styles.addressClusterFull,
        variant === "plain" && styles.addressClusterPlain,
        className,
      )}
      onMouseEnter={() => onHoverAddressChange?.(address)}
      onMouseLeave={() => onHoverAddressChange?.(undefined)}
    >
      {copyPlacement === "left" && copyButton}
      {addressWithTooltip}
      {copyPlacement === "right" && copyButton}
    </span>
  )
}

function AddressChipTooltip({
  address,
  displayAddress,
  onCopyError,
  tooltipVariants,
}: {
  readonly address: string
  readonly displayAddress: string
  readonly onCopyError?: (error: unknown) => void
  readonly tooltipVariants?: readonly AddressChipTooltipVariant[]
}) {
  const hasDistinctDisplayAddress = displayAddress !== address
  const hasTooltipVariants = tooltipVariants !== undefined && tooltipVariants.length > 0

  return (
    <span className={styles.tooltip}>
      {hasTooltipVariants ? (
        tooltipVariants.map(variant => (
          <AddressChipTooltipRow
            key={`${variant.label}:${variant.value}`}
            label={variant.label}
            copyLabel={variant.label.toLowerCase()}
            value={variant.value}
            onCopyError={onCopyError}
          />
        ))
      ) : (
        <AddressChipTooltipRow
          label={hasDistinctDisplayAddress ? "Address" : "Raw address"}
          copyLabel={hasDistinctDisplayAddress ? "address" : "raw address"}
          value={displayAddress}
          onCopyError={onCopyError}
        />
      )}
      {hasTooltipVariants || hasDistinctDisplayAddress ? (
        <AddressChipTooltipRow
          label="Raw address"
          copyLabel="raw address"
          value={address}
          onCopyError={onCopyError}
        />
      ) : null}
    </span>
  )
}

function AddressChipTooltipRow({
  copyLabel,
  label,
  value,
  onCopyError,
}: {
  readonly copyLabel: string
  readonly label: string
  readonly onCopyError?: (error: unknown) => void
  readonly value: string
}) {
  return (
    <span className={styles.tooltipRow}>
      <span>{label}</span>
      <span className={styles.tooltipCopyValue}>
        <code>{value}</code>
        <CopyInlineAction
          copiedLabel={`${copyLabel} copied`}
          label={`Copy ${copyLabel}`}
          onCopyError={onCopyError}
          size="compact"
          value={value}
        />
      </span>
    </span>
  )
}

function getDisplayAddress(
  address: string,
  formatAddress: AddressChipAddressFormatter | undefined,
): string {
  if (!formatAddress) return address

  try {
    return formatAddress(address)
  } catch {
    return address
  }
}

function shortenAddress(address: string): string {
  if (address.includes(":")) {
    const [workchain, hash] = address.split(":")
    return `${workchain}:${shortenMiddle(hash, {start: 6, end: 6})}`
  }

  return shortenMiddle(address, {start: 6, end: 6})
}
