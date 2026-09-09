import {AddressChip, InfoPopover} from "@acton/ui"

import {normalizeAddress, toRawAddress} from "./utils"
import styles from "./AddressFormats.module.css"

/** Shows both friendly network encodings and the raw identity in the address converter. */
export function AddressFormats({
  address,
  onAddressClick,
}: {
  readonly address: string
  readonly onAddressClick: (address: string, testOnly: boolean | undefined) => void
}) {
  const networks = [
    {label: "Mainnet", testOnly: false},
    {label: "Testnet", testOnly: true},
  ]
  const formats: {label: string; value: string; description: string; testOnly?: boolean}[] =
    networks.flatMap(network => [
      {
        testOnly: network.testOnly,
        label: `${network.label} bounceable`,
        description: `Tells a wallet to enable bounce: if the recipient cannot process the transfer, remaining funds can return to the sender${network.testOnly ? "; marked as testnet-only so mainnet wallets should reject it" : ""}`,
        value: normalizeAddress(address, {
          testOnly: network.testOnly,
          bounceable: true,
        }),
      },
      {
        testOnly: network.testOnly,
        label: `${network.label} non-bounceable`,
        description: `Tells a wallet to disable bounce: funds can reach an uninitialized account instead of being returned${network.testOnly ? "; marked as testnet-only so mainnet wallets should reject it" : ""}`,
        value: normalizeAddress(address, {
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
    <dl className={styles.formats}>
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
              onAddressClick={() => onAddressClick(value, testOnly)}
            />
          </dd>
        </div>
      ))}
    </dl>
  )
}
