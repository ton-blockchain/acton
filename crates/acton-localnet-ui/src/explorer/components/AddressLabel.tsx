import type {FC} from "react"

import {useAddressName} from "../hooks/useAddressBook"
import {useAddressFormat} from "../hooks/useNetworkInfo"

import {formatAddress} from "./utils"

interface AddressLabelProps {
  readonly address: string
  readonly shorten?: boolean
  readonly fallback?: string
  readonly nameFallback?: string
  readonly className?: string
}

export const AddressLabel: FC<AddressLabelProps> = ({
  address,
  shorten = true,
  fallback = "Unknown",
  nameFallback,
  className,
}) => {
  const addressFormat = useAddressFormat()
  const name = useAddressName(address)

  if (!address) {
    return <span className={className}>{fallback}</span>
  }

  const label = name || nameFallback || formatAddress(address, shorten, addressFormat)
  return <span className={className}>{label}</span>
}
