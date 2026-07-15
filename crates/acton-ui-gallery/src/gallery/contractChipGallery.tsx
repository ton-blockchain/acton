import {useState} from "react"

import {ContractChip, type ContractChipData} from "@acton/ui"

import styles from "./contractChipGallery.module.css"
import type {ComponentGallery} from "./types"

const TREASURY_ADDRESS = "0:8d8f6f75a9a2e6fae93c4256e33d5b99e0d60f39d1ff0a31ecf79a833f4df47e"
const UNKNOWN_ADDRESS = "0:5f6d7e8c9b0a1234567890abcdef1234567890abcdef1234567890abcdef1234"
const FRIENDLY_TREASURY_ADDRESS = "EQCNY9vdqaLm-uk8QlbjPVuZ4NYPOdH_CjHs95qDP030fg"

const contracts = new Map<string, ContractChipData>([
  [TREASURY_ADDRESS, {displayName: "Treasury", letter: "T"}],
])

function formatFriendlyAddress(address: string): string {
  return address === TREASURY_ADDRESS ? FRIENDLY_TREASURY_ADDRESS : address
}

function chipSample(label: string, chip: React.ReactNode, description: string) {
  return (
    <article className={styles.sample}>
      <div className={styles.sampleHeader}>
        <h4 className={styles.sampleTitle}>{label}</h4>
        <p className={styles.sampleDescription}>{description}</p>
      </div>
      <div className={styles.chipLine}>{chip}</div>
    </article>
  )
}

function InteractiveContractSample() {
  const [openedAddress, setOpenedAddress] = useState<string>()

  return (
    <div className={styles.interactiveSample}>
      <ContractChip
        address={TREASURY_ADDRESS}
        contracts={contracts}
        formatAddress={formatFriendlyAddress}
        onContractClick={setOpenedAddress}
      />
      <span className={styles.interactionResult} aria-live="polite">
        {openedAddress
          ? `Opened ${openedAddress}`
          : "Click the contract name to inspect navigation"}
      </span>
    </div>
  )
}

export const contractChipGallery = {
  id: "contract-chip",
  title: "ContractChip",
  status: "ready",
  summary:
    "ContractChip identifies known contracts, keeps unknown addresses readable, and provides a shared copy action without depending on ABI types.",
  importStatement: 'import {ContractChip} from "@acton/ui"',
  agentSummary:
    "Use ContractChip for a TON address when optional contract metadata can provide a short letter and display name. Pass a formatter callback when the application owns network-specific address formatting.",
  usage: [
    "Pass only the minimal contract metadata map: displayName and letter.",
    "Use formatAddress to convert raw addresses into the network-specific display form without adding @ton/core to the UI package.",
    "Provide onContractClick when the chip should navigate; the copy action remains independent.",
  ],
  avoid: [
    "Do not pass ABI objects or parser models to ContractChip.",
    "Do not add a second copy button around the chip.",
    "Do not pre-truncate known contract names or addresses; let the component own its compact display.",
  ],
  sections: [
    {
      id: "contract-chip-states",
      title: "Identity States",
      description:
        "Missing, unresolved, and resolved addresses exercise every non-interactive presentation state.",
      content: (
        <div className={styles.grid}>
          {chipSample(
            "Unavailable",
            <ContractChip address={undefined} />,
            "No address is available.",
          )}
          {chipSample(
            "Unknown Contract",
            <ContractChip address={UNKNOWN_ADDRESS} />,
            "An address without registered metadata uses the question-mark identity.",
          )}
          {chipSample(
            "Known Contract",
            <ContractChip address={TREASURY_ADDRESS} contracts={contracts} />,
            "Known metadata adds the contract letter and display name.",
          )}
        </div>
      ),
    },
    {
      id: "contract-chip-formatting",
      title: "Address Formatting",
      description:
        "Formatting stays caller-owned, so the base package does not depend on TON address or ABI libraries.",
      content: (
        <div className={styles.formatPanel}>
          <span className={styles.formatLabel}>raw → friendly</span>
          <ContractChip
            address={TREASURY_ADDRESS}
            contracts={contracts}
            formatAddress={formatFriendlyAddress}
          />
        </div>
      ),
    },
    {
      id: "contract-chip-interactive",
      title: "Navigation and Copy",
      description:
        "The contract name is a real navigation button while the compact copy action remains its sibling, avoiding nested interactive elements.",
      content: <InteractiveContractSample />,
    },
  ],
} satisfies ComponentGallery
