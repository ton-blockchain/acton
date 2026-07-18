import {Check, Copy} from "lucide-react"
import type {MouseEvent, ReactNode} from "react"

import {cx} from "../../lib/cx"
import {useCopyValue} from "../../lib/useCopyValue"
import {InlineAction} from "../InlineActions/InlineActions"

import styles from "./AddressChip.module.css"

export type AddressChipCopyPlacement = "left" | "right"
export type AddressChipVariant = "accent" | "plain"
export type AddressChipAddressFormatter = (address: string) => string

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
        title={displayAddress}
        onClick={event => {
          event.stopPropagation()
          onAddressClick(address, event)
        }}
      >
        {label ?? addressContent}
      </button>
    ) : (
      <span className={addressClassName} title={displayAddress || undefined}>
        {label ?? addressContent}
      </span>
    )

  if (!address) return addressNode

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
      {addressNode}
      {copyPlacement === "right" && copyButton}
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
    return `${workchain}:${hash.slice(0, 6)}…${hash.slice(-6)}`
  }

  if (address.length > 12) return `${address.slice(0, 6)}…${address.slice(-6)}`
  return address
}
