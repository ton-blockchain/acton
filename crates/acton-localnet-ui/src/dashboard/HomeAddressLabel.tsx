import * as React from "react"

import {formatAddress, parseAddress} from "../explorer/components/utils"
import {useAddressName} from "../explorer/hooks/useAddressBook"

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
  const normalizedAddress = React.useMemo(() => {
    const parsed = address ? parseAddress(address) : undefined
    return parsed?.toString({testOnly: true})
  }, [address])
  const name = useAddressName(normalizedAddress ?? "")

  if (!normalizedAddress) {
    return <span className={className}>{fallback}</span>
  }

  const shortAddress = formatAddress(normalizedAddress)
  const fullAddress = formatAddress(normalizedAddress, false)

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
