import type React from "react"
import { useAddressName } from "../hooks/useAddressBook"
import { formatAddress } from "./utils"

interface AddressLabelProps {
  address: string
  shorten?: boolean
  forceReal?: boolean
  fallback?: string
  className?: string
}

export const AddressLabel: React.FC<AddressLabelProps> = ({
  address,
  shorten = true,
  forceReal = false,
  fallback = "Unknown",
  className,
}) => {
  const name = useAddressName(address)

  if (!address) {
    return <span className={className}>{fallback}</span>
  }

  const label = name || formatAddress(address, shorten, forceReal)
  return <span className={className}>{label}</span>
}
