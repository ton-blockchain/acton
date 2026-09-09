import {AddressChip, InfoPopover} from "@acton/ui"

import {normalizeAddress, toRawAddress, type AddressFormatOptions} from "./utils"
import styles from "./AddressFormats.module.css"

/** Shared address representations for account popovers and the standalone converter. */
export function AddressFormats({
  address,
  addressFormat,
  variant = "compact",
  includeTestnet = false,
  onAddressClick,
}: {
  readonly address: string
  readonly addressFormat: AddressFormatOptions
  readonly variant?: "compact" | "table"
  readonly includeTestnet?: boolean
  readonly onAddressClick?: (address: string, testOnly: boolean | undefined) => void
}) {
  const networks = includeTestnet
    ? [
        {label: "Mainnet", testOnly: false},
        {label: "Testnet", testOnly: true},
      ]
    : [{label: "", testOnly: addressFormat.testOnly}]
  const formats: {label: string; value: string; description: string; testOnly?: boolean}[] =
    networks.flatMap(network => [
      {
        testOnly: network.testOnly,
        label: network.label ? `${network.label} bounceable` : "Bounceable",
        description: `Tells a wallet to enable bounce: if the recipient cannot process the transfer, remaining funds can return to the sender${network.testOnly ? "; marked as testnet-only so mainnet wallets should reject it" : ""}`,
        value: normalizeAddress(address, {
          ...addressFormat,
          testOnly: network.testOnly,
          bounceable: true,
        }),
      },
      {
        testOnly: network.testOnly,
        label: network.label ? `${network.label} non-bounceable` : "Non-bounceable",
        description: `Tells a wallet to disable bounce: funds can reach an uninitialized account instead of being returned${network.testOnly ? "; marked as testnet-only so mainnet wallets should reject it" : ""}`,
        value: normalizeAddress(address, {
          ...addressFormat,
          testOnly: network.testOnly,
          bounceable: false,
        }),
      },
    ])
  formats.push({
    label: "Raw",
    value: toRawAddress(address),
    description:
      "Workchain and 256-bit account ID in hex, without bounce, testnet-only or checksum information",
  })

  return (
    <dl className={styles.formats} data-variant={variant}>
      {formats.map(({label, value, testOnly, description}) => (
        <div className={styles.row} key={label}>
          <dt className={styles.label}>
            {label}
            <InfoPopover ariaLabel={`About ${label.toLowerCase()}`} placement="top" openDelay={0}>
              {description}
            </InfoPopover>
          </dt>
          <dd className={styles.valueRow}>
            <AddressChip
              address={toRawAddress(address)}
              formatAddress={() => value}
              shorten={false}
              onAddressClick={onAddressClick ? () => onAddressClick(value, testOnly) : undefined}
            />
          </dd>
        </div>
      ))}
    </dl>
  )
}
