import * as React from "react"

import {formatAddress, normalizeAddress, parseAddress} from "../explorer/components/utils"
import {useAddressName} from "../explorer/hooks/useAddressBook"
import {useAddressFormat} from "../explorer/hooks/useNetworkInfo"

import styles from "./DashboardPage.module.css"

interface HomeAddressLabelProps {
  readonly address?: string
  readonly fallback?: string
  readonly className?: string
}

export const HomeAddressLabel: React.FC<HomeAddressLabelProps> = ({
  address,
  fallback = "Unknown",
  className,
}) => {
  const addressFormat = useAddressFormat()
  const normalizedAddress = React.useMemo(() => {
    if (!address) {
      return
    }

    const parsed = parseAddress(address)
    if (!parsed) {
      return
    }

    return normalizeAddress(address, addressFormat)
  }, [address, addressFormat])
  const name = useAddressName(normalizedAddress ?? "")

  if (!normalizedAddress) {
    return <span className={className}>{fallback}</span>
  }

  const shortAddress = formatAddress(normalizedAddress, true, addressFormat)
  const fullAddress = formatAddress(normalizedAddress, false, addressFormat)

  if (!name) {
    return (
      <span className={`${styles.addressDisplay} ${className ?? ""}`} title={fullAddress}>
        {shortAddress}
      </span>
    )
  }

  return (
    <span
      className={`${styles.addressDisplay} ${className ?? ""}`}
      title={`${name} (${fullAddress})`}
    >
      <span className={styles.addressDisplayName}>{name}</span>
      <span className={styles.addressDisplaySeparator}>·</span>
      <span className={styles.addressDisplayAddress}>{shortAddress}</span>
    </span>
  )
}
