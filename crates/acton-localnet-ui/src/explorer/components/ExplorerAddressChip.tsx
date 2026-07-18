import type {FC} from "react"

import {AddressChip, type AddressChipCopyPlacement, type AddressChipVariant} from "@acton/ui"

import {useAddressName} from "../hooks/useAddressBook"
import type {ExplorerNavigationClickEvent} from "../hooks/useOpenExplorerPath"
import {useAddressFormat} from "../hooks/useNetworkInfo"

import {formatAddress} from "./utils"

type ExplorerAddressChipDisplayFormat = "network" | "raw"

interface ExplorerAddressChipProps {
  readonly address: string
  readonly fallback?: string
  readonly copiedAddress?: string
  readonly highlighted?: boolean
  readonly copyable?: boolean
  readonly copyPlacement?: AddressChipCopyPlacement
  readonly displayFormat?: ExplorerAddressChipDisplayFormat
  readonly shorten?: boolean
  readonly resolveName?: boolean
  readonly nameFallback?: string
  readonly onAddressClick?: (address: string, event?: ExplorerNavigationClickEvent) => void
  readonly onCopyAddress?: (address: string) => Promise<void> | void
  readonly onHoverAddressChange?: (address: string | undefined) => void
  readonly variant?: AddressChipVariant
}

export const ExplorerAddressChip: FC<ExplorerAddressChipProps> = ({
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
  variant,
}) => {
  const addressFormat = useAddressFormat()
  const resolvedName = useAddressName(resolveName ? address : "")
  return (
    <AddressChip
      address={address}
      copied={copiedAddress === address}
      copyable={copyable}
      copyPlacement={copyPlacement}
      fallback={fallback}
      formatAddress={
        displayFormat === "raw" ? undefined : value => formatAddress(value, false, addressFormat)
      }
      highlighted={highlighted}
      label={resolveName ? resolvedName || nameFallback : undefined}
      onAddressClick={onAddressClick}
      onCopyAddress={onCopyAddress}
      onCopyError={error => console.error("Failed to copy address", error)}
      onHoverAddressChange={onHoverAddressChange}
      shorten={shorten}
      variant={variant}
    />
  )
}
