import {ParsedBodySection, type ContractChipData, type ParsedTransactionBody} from "@acton/ui"

import styles from "./parsedBodySectionGallery.module.css"
import type {ComponentGallery} from "./types"

const RECEIVER_ADDRESS = "0:f76018765faedb4d9d4c9e1be0b08772f6d8859a30cf9a7af5016d72fd70ac99"
const contracts = new Map<string, ContractChipData>([
  [RECEIVER_ADDRESS, {displayName: "Receiver", letter: "R"}],
])

const transferBody: ParsedTransactionBody = {
  name: "TokenTransfer",
  value: {
    kind: "object",
    entries: [
      {key: "queryId", value: {kind: "scalar", value: "482109372"}},
      {key: "amount", value: {kind: "scalar", value: "2500000000", typeName: "coins"}},
      {key: "destination", value: {kind: "address", value: RECEIVER_ADDRESS}},
      {key: "notify", value: {kind: "boolean", value: true}},
      {
        key: "payload",
        value: {
          kind: "object",
          typeName: "ForwardPayload",
          entries: [
            {key: "opcode", value: {kind: "scalar", value: "1936289396"}},
            {key: "comment", value: {kind: "scalar", value: "release payment"}},
          ],
        },
      },
    ],
  },
}

export const parsedBodySectionGallery = {
  id: "parsed-body-section",
  title: "ParsedBodySection",
  status: "ready",
  summary:
    "ParsedBodySection adds an accessible disclosure around ParsedValueView while accepting the same minimal ABI-independent value tree.",
  importStatement: 'import {ParsedBodySection} from "@acton/ui"',
  agentSummary:
    "Use ParsedBodySection for decoded message bodies and similar expandable technical trees. Domain code remains responsible for parsing and passes only the body name and ParsedValue root.",
  usage: [
    "Pass defaultExpanded only when the surrounding context benefits from immediately visible decoded data.",
    "Use title to reuse the disclosure presentation for another already-decoded body-like value.",
    "Pass the same contract metadata and address formatter used by standalone ParsedValueView.",
  ],
  avoid: [
    "Do not perform ABI lookup or decoding inside ParsedBodySection.",
    "Do not add another disclosure button around the component.",
    "Do not render the component for missing data; an undefined parsedBody intentionally returns no markup.",
  ],
  sections: [
    {
      id: "parsed-body-disclosure",
      title: "Disclosure States",
      description:
        "Collapsed and expanded instances verify label, chevron, spacing, focus, and the decoded tree together.",
      content: (
        <div className={styles.stateGrid}>
          <article className={styles.stateSample}>
            <span className={styles.stateLabel}>Collapsed</span>
            <ParsedBodySection parsedBody={transferBody} contracts={contracts} />
          </article>
          <article className={styles.stateSample}>
            <span className={styles.stateLabel}>Expanded</span>
            <ParsedBodySection parsedBody={transferBody} contracts={contracts} defaultExpanded />
          </article>
        </div>
      ),
    },
    {
      id: "parsed-body-custom-title",
      title: "Reusable Label",
      description:
        "The disclosure can label another pre-decoded value without changing its parsing or recursive renderer.",
      content: (
        <div className={styles.wideSample}>
          <ParsedBodySection
            title="Decoded Storage"
            parsedBody={transferBody}
            contracts={contracts}
            defaultExpanded
          />
        </div>
      ),
    },
  ],
} satisfies ComponentGallery
