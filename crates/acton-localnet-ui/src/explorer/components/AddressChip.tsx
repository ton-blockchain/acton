import {Check, Copy} from "lucide-react"
import {useEffect, useState} from "react"
import type {FC} from "react"

import type {ExplorerNavigationClickEvent} from "../hooks/useOpenExplorerPath"
import {useAddressFormat} from "../hooks/useNetworkInfo"

import {AddressLabel} from "./AddressLabel"
import {formatAddress} from "./utils"

import styles from "./AddressChip.module.css"

type AddressChipCopyPlacement = "left" | "right"
type AddressChipDisplayFormat = "network" | "raw"

interface AddressChipProps {
  readonly address: string
  readonly fallback?: string
  readonly copiedAddress?: string
  readonly highlighted?: boolean
  readonly copyable?: boolean
  readonly copyPlacement?: AddressChipCopyPlacement
  readonly displayFormat?: AddressChipDisplayFormat
  readonly shorten?: boolean
  readonly resolveName?: boolean
  readonly nameFallback?: string
  readonly onAddressClick?: (address: string, event?: ExplorerNavigationClickEvent) => void
  readonly onCopyAddress?: (address: string) => Promise<void> | void
  readonly onHoverAddressChange?: (address: string | undefined) => void
}

export const AddressChip: FC<AddressChipProps> = ({
  address,
  fallback,
  copiedAddress,
  highlighted = false,
  copyable = true,
  copyPlacement = "right",
  displayFormat = "network",
  shorten = true,
  resolveName = true,
  nameFallback,
  onAddressClick,
  onCopyAddress,
  onHoverAddressChange,
}) => {
  const addressFormat = useAddressFormat()
  const [isCopiedInternally, setIsCopiedInternally] = useState(false)
  const isCopied = copiedAddress === address || isCopiedInternally
  const fullAddress = address
    ? displayFormat === "raw"
      ? address
      : formatAddress(address, false, addressFormat)
    : ""
  const addressContent = address
    ? shorten
      ? formatChipAddress(fullAddress)
      : fullAddress
    : (fallback ?? "Unknown")
  const addressLabel =
    address && resolveName ? (
      <AddressLabel address={address} fallback={addressContent} nameFallback={nameFallback} />
    ) : (
      addressContent
    )
  const addressClassName = `${onAddressClick ? styles.addressButton : styles.addressText} ${
    highlighted ? styles.addressHighlighted : ""
  }`
  const fullAddressClassName = shorten ? "" : styles.addressFull

  useEffect(() => {
    if (!isCopiedInternally) {
      return
    }

    const timer = globalThis.setTimeout(() => setIsCopiedInternally(false), 1600)
    return () => globalThis.clearTimeout(timer)
  }, [isCopiedInternally])

  const copyAddress = async () => {
    if (!address) {
      return
    }

    try {
      if (onCopyAddress) {
        await onCopyAddress(address)
      } else {
        await globalThis.navigator.clipboard.writeText(fullAddress)
      }
      setIsCopiedInternally(true)
    } catch (error) {
      console.error("Failed to copy address", error)
    }
  }

  const addressNode = onAddressClick ? (
    <button
      type="button"
      className={`${addressClassName} ${fullAddressClassName}`}
      title={fullAddress}
      onClick={event => {
        event.stopPropagation()
        onAddressClick(address, event)
      }}
    >
      {addressLabel}
    </button>
  ) : (
    <span className={`${addressClassName} ${fullAddressClassName}`} title={fullAddress}>
      {addressLabel}
    </span>
  )

  if (!address) {
    return addressNode
  }

  const copyButton = (
    <button
      type="button"
      className={`${styles.copyButton} ${isCopied ? styles.copyButtonCopied : ""}`}
      onClick={event => {
        event.stopPropagation()
        void copyAddress()
      }}
      aria-label={isCopied ? "Address copied" : "Copy address"}
      title={isCopied ? "Copied" : "Copy address"}
    >
      {isCopied ? <Check size={13} /> : <Copy size={13} />}
    </button>
  )

  return (
    <span
      className={`${styles.addressCluster} ${shorten ? "" : styles.addressClusterFull}`}
      onMouseEnter={() => onHoverAddressChange?.(address)}
      onMouseLeave={() => onHoverAddressChange?.(undefined)}
    >
      {copyable && copyPlacement === "left" && copyButton}
      {addressNode}
      {copyable && copyPlacement === "right" && copyButton}
    </span>
  )
}

function formatChipAddress(address: string): string {
  if (!address) {
    return "Unknown"
  }

  if (address.includes(":")) {
    const [workchain, hash] = address.split(":")
    return `${workchain}:${hash.slice(0, 6)}…${hash.slice(-6)}`
  }

  if (address.length > 12) {
    return `${address.slice(0, 6)}…${address.slice(-6)}`
  }

  return address
}
