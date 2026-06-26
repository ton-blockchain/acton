import {Check, Copy} from "lucide-react"
import {useEffect, useState} from "react"
import type {FC} from "react"

import type {ExplorerNavigationClickEvent} from "../hooks/useOpenExplorerPath"
import {useAddressFormat} from "../hooks/useNetworkInfo"

import {AddressLabel} from "./AddressLabel"
import {formatAddress} from "./utils"

import styles from "./AddressChip.module.css"

type AddressChipCopyPlacement = "left" | "right"

interface AddressChipProps {
  readonly address: string
  readonly fallback?: string
  readonly copiedAddress?: string
  readonly highlighted?: boolean
  readonly copyPlacement?: AddressChipCopyPlacement
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
  copyPlacement = "right",
  resolveName = true,
  nameFallback,
  onAddressClick,
  onCopyAddress,
  onHoverAddressChange,
}) => {
  const addressFormat = useAddressFormat()
  const [isCopiedInternally, setIsCopiedInternally] = useState(false)
  const isCopied = copiedAddress === address || isCopiedInternally
  const fullAddress = address ? formatAddress(address, false, addressFormat) : ""
  const addressContent = address
    ? formatAddress(address, true, addressFormat)
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
      className={addressClassName}
      title={fullAddress}
      onClick={event => {
        event.stopPropagation()
        onAddressClick(address, event)
      }}
    >
      {addressLabel}
    </button>
  ) : (
    <span className={addressClassName} title={fullAddress}>
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
      className={styles.addressCluster}
      onMouseEnter={() => onHoverAddressChange?.(address)}
      onMouseLeave={() => onHoverAddressChange?.(undefined)}
    >
      {copyPlacement === "left" && copyButton}
      {addressNode}
      {copyPlacement === "right" && copyButton}
    </span>
  )
}
