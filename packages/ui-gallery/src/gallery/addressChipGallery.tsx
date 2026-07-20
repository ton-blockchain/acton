import {AddressChip} from "@acton/ui"
import type {ReactNode} from "react"

import styles from "./addressChipGallery.module.css"
import type {ComponentGallery} from "./types"

const RAW_ADDRESS = "0:1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef"
const FRIENDLY_ADDRESS = "EQASNFZ4mrze8SNFZ4mrze8SNFZ4mrze8SNFZ4mrze8SNFZ4"

function sample(title: string, content: ReactNode) {
  return (
    <article className={styles.sample}>
      <span className={styles.sampleTitle}>{title}</span>
      {content}
    </article>
  )
}

export const addressChipGallery = {
  id: "address-chip",
  title: "AddressChip",
  status: "ready",
  summary:
    "AddressChip renders compact technical addresses with optional caller-owned labels, navigation, copy feedback, highlighting, and address formatting.",
  importStatement: 'import {AddressChip} from "@acton/ui"',
  agentSummary:
    "Use AddressChip for a compact address value that may be copied or opened. Keep TON parsing, network formatting, and name resolution in the caller.",
  usage: [
    "Pass formatAddress when the application needs network-specific friendly formatting.",
    "Pass label when a resolved wallet or contract name should replace the shortened address.",
    'Use variant="plain" for neutral text without address hover styling and with an always-visible copy action.',
    "Use copyPlacement only when table geometry requires the copy action on the left.",
    "Use highlighted to coordinate related addresses across rows or trace nodes.",
  ],
  avoid: [
    "Do not pass TON Address objects or address-book services into AddressChip.",
    "Do not wrap AddressChip in another copy button.",
    "Do not pre-truncate the address; let AddressChip own compact formatting.",
  ],
  sections: [
    {
      id: "address-chip-states",
      title: "Display States",
      description: "Fallback, raw address, resolved label, and coordinated highlight states.",
      content: (
        <div className={styles.grid}>
          {sample("Unavailable", <AddressChip address={undefined} fallback="Unknown account" />)}
          {sample("Address only", <AddressChip address={FRIENDLY_ADDRESS} copyable={false} />)}
          {sample(
            "Resolved label",
            <AddressChip address={FRIENDLY_ADDRESS} label={<span>Deployer wallet</span>} />,
          )}
          {sample("Plain", <AddressChip address={FRIENDLY_ADDRESS} variant="plain" />)}
          {sample(
            "Highlighted",
            <AddressChip address={FRIENDLY_ADDRESS} highlighted copyable={false} />,
          )}
        </div>
      ),
    },
    {
      id: "address-chip-formatting",
      title: "Formatting and Actions",
      description:
        "Formatting remains caller-owned while navigation and copy stay independent sibling controls.",
      content: (
        <div className={styles.actionPanel}>
          <AddressChip
            address={RAW_ADDRESS}
            copyPlacement="left"
            formatAddress={() => FRIENDLY_ADDRESS}
            onAddressClick={() => undefined}
          />
          <AddressChip address={RAW_ADDRESS} shorten={false} copyable={false} />
        </div>
      ),
    },
  ],
} satisfies ComponentGallery
