import type React from "react"

import {useAddressName} from "../hooks/useAddressBook"

import {formatAddress} from "./utils"

interface AddressLabelProps {
  readonly address: string
  readonly shorten?: boolean
  readonly fallback?: string
  readonly className?: string
}

export const AddressLabel: React.FC<AddressLabelProps> = ({
  address,
  shorten = true,
  fallback = "Unknown",
  className,
}) => {
  const name = useAddressName(address)

  if (!address) {
    return <span className={className}>{fallback}</span>
  }

  const label = name || formatAddress(address, shorten)
  return <span className={className}>{label}</span>
}
