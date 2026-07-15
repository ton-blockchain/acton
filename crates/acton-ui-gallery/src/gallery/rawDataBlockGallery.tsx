import {ContentTabs, DisclosureToggle, RawDataBlock, type ContentTab} from "@acton/ui"
import {useState} from "react"

import styles from "./rawDataBlockGallery.module.css"
import type {ComponentGallery} from "./types"

type RawTab = "disasm" | "base64" | "hex" | "hex-hash" | "base64-hash"

const rawTabs = [
  {value: "disasm", label: "disasm"},
  {value: "base64", label: "base64"},
  {value: "hex", label: "hex"},
  {value: "hex-hash", label: "hex hash"},
  {value: "base64-hash", label: "base64 hash"},
] satisfies readonly ContentTab<RawTab>[]

const disasmValue = [
  "SETCP 0",
  "DICTPUSHCONST 19 [",
  "    0 => {",
  "        DUP",
  "        SBITS",
  "        LESSINT 32",
  "        PUSHCONT_SHORT {",
  "            DROP2",
  "        }",
  "        PUSHCONT {",
  "            DUP",
  "            PLDU 32",
  "            DUP",
  "            PUSHINT_LONG 1702392942",
  "            NEQ",
  "            OVER",
  "            PUSHINT_LONG 1936289396",
  "            NEQ",
  "            AND",
  "        }",
  "    }",
].join("\n")

const encodedValues = {
  base64:
    "te6cckECFAEAAoEAART/APSKE/S88sgLAQIBIAINAgFIAwQC3NAg10nBIJFbj2Mg1wsfIIIQZXh0br0hghBzaw50vbCSXwPgghBIeHRuuo60gCDXIQHQdNch+kAw+kT4KPpEMFi9KQgQFB1xj0BYMH9A5voTGRMOGAQNchcH/bPOAxINdJgQKAuZEw4HDIEA8CASAFDAIBIAYJAgFuCAsIAgEgDA0CASAOEQIBIBIWAQL6ExQW8L/8vACP0BQFjGhCQRrkQGFMxgGXB9shaBI=",
  hex: "B5EE9C7241010101002F00005A420002013403010101C0020043CA015800F8A7EA5004FA40ED44D0D31F89F0302702030101810105",
  "hex-hash": "65a184650d89a7a435714780a2f6084a8b1c11180c76672cc54f5c6412a23fc0",
  "base64-hash": "ZaGEZQ2Jp6Q1cUeAovYISoscERgMdmcsxU9cZBKiP8A=",
} satisfies Record<Exclude<RawTab, "disasm">, string>

const longPayload = [
  encodedValues.base64,
  "gCDXIQHQdNch+kAw+kT4KPpEMFi9kVvg7UTQgQFB1yH0BYMH9A5voTGRMOGAQNchcH/bPOAxINdJgQKAuZEw4HDIEA8CASAFDAIBIAYJAgFu",
  "BwqAGa30dqJoQCDJrkOuF/8AAGa8d9qJoQBDJrkOuFj8ACAUgKCwAXsyX7UTQcdch1wsfgABGyYvtRNDXCgCAAGb5fD2omhAqKDrkPoCwBAv10",
].join("")

const longLog = [
  "0000000000000000  SETCP 0",
  "0000000000000001  DICTPUSHCONST 19 [",
  "0000000000000002    0 => { DUP SBITS LESSINT 32 PUSHCONT_SHORT { DROP2 } }",
  "0000000000000003  PUSHINT_LONG 1702392942",
  "0000000000000004  PUSHINT_LONG 1936289396",
  "0000000000000005  EQUAL",
  "0000000000000006  IFJMP target:ffffffffffffffffffffffffffffffffffffffff",
].join("\n")

function EmbeddedTabsSample() {
  const [expanded, setExpanded] = useState(true)
  const [activeTab, setActiveTab] = useState<RawTab>("base64")
  const value = activeTab === "disasm" ? disasmValue : encodedValues[activeTab]

  return (
    <div className={styles.stack}>
      <div className={styles.inlineHeader}>
        <span>Code</span>
        <DisclosureToggle
          expanded={expanded}
          contextLabel="code"
          onClick={() => setExpanded(current => !current)}
        />
      </div>

      {expanded && (
        <ContentTabs
          ariaLabel="Raw code formats"
          tabs={rawTabs}
          value={activeTab}
          onValueChange={setActiveTab}
          panelClassName={styles.tabsPanel}
        >
          <RawDataBlock
            variant="embedded"
            value={value}
            copyLabel={activeTab}
            wrap={activeTab !== "disasm"}
          >
            {activeTab === "disasm" ? <DisasmPreview /> : undefined}
          </RawDataBlock>
        </ContentTabs>
      )}
    </div>
  )
}

function StandaloneSamples() {
  return (
    <div className={styles.sampleRow}>
      <div>
        <h4 className={styles.sampleTitle}>Wrapped payload</h4>
        <RawDataBlock
          value={longPayload}
          copyLabel="raw body"
          maxHeight="12rem"
          variant="standalone"
        />
      </div>
      <div>
        <h4 className={styles.sampleTitle}>No wrap log</h4>
        <RawDataBlock
          value={longLog}
          copyLabel="VM log"
          maxHeight="12rem"
          variant="standalone"
          wrap={false}
        />
      </div>
    </div>
  )
}

function CollapsibleSamples() {
  const [expanded, setExpanded] = useState(true)

  return (
    <div className={styles.sampleRow}>
      <RawDataBlock
        title="Raw message body"
        titleLabel="raw message body"
        collapsible
        expanded={expanded}
        onExpandedChange={setExpanded}
        value={longPayload}
        copyLabel="raw message body"
        maxHeight="10rem"
      />
      <RawDataBlock
        title="State init"
        collapsible
        defaultExpanded={false}
        value={encodedValues.hex}
        copyLabel="state init"
        maxHeight="10rem"
      />
    </div>
  )
}

function EmptySamples() {
  return (
    <div className={styles.sampleRow}>
      <RawDataBlock
        title="VM log"
        value=""
        empty
        emptyContent={"No VM logs were collected for this trace.\nRe-run with --verbose flag."}
      />
      <RawDataBlock
        title="Executor log"
        value=""
        empty
        emptyContent={
          <div className={styles.emptyDetails}>
            <span>No executor logs were collected.</span>
            <span>Enable verbose trace collection to see executor output here.</span>
          </div>
        }
      />
    </div>
  )
}

function DisasmPreview() {
  return (
    <>
      <span className={styles.keyword}>SETCP</span> <span className={styles.number}>0</span>
      {"\n"}
      <span className={styles.keyword}>DICTPUSHCONST</span>{" "}
      <span className={styles.number}>19</span> [ {"\n"}
      {"    "}
      <span className={styles.number}>0</span> =&gt; {"{\n"}
      {"        "}
      <span className={styles.keyword}>DUP</span>
      {"\n"}
      {"        "}
      <span className={styles.keyword}>SBITS</span>
      {"\n"}
      {"        "}
      <span className={styles.keyword}>LESSINT</span> <span className={styles.number}>32</span>
      {"\n"}
      {"        "}
      <span className={styles.keyword}>PUSHCONT_SHORT</span> {"{\n"}
      {"            "}
      <span className={styles.keyword}>DROP2</span>
      {"\n"}
      {"        }"}
      {"\n"}
      {"        "}
      <span className={styles.keyword}>PUSHCONT</span> {"{\n"}
      {"            "}
      <span className={styles.keyword}>DUP</span>
      {"\n"}
      {"            "}
      <span className={styles.keyword}>PLDU</span> <span className={styles.number}>32</span>
      {"\n"}
      {"            "}
      <span className={styles.keyword}>PUSHINT_LONG</span>{" "}
      <span className={styles.number}>1702392942</span>
      {"\n"}
      {"            "}
      <span className={styles.keyword}>NEQ</span>
      {"\n"}
      {"        }"}
      {"\n"}
      {"    }"}
    </>
  )
}

export const rawDataBlockGallery = {
  id: "raw-data-block",
  title: "RawDataBlock",
  status: "ready",
  summary:
    "RawDataBlock renders large raw values, code-like payloads, and encoded data with scroll, wrapping, max-height, and copy behavior.",
  importStatement: 'import { RawDataBlock } from "@acton/ui"',
  agentSummary:
    "Use RawDataBlock for the content inside raw-data tabs or standalone payload viewers. Keep tab state in ContentTabs and decoding outside the component.",
  usage: [
    "Use for base64, hex, hashes, VM logs, disassembly output, and raw message bodies.",
    'Use variant="embedded" inside ContentTabs so there is only one visible panel frame.',
    "Use wrap={false} for disassembly, logs, and aligned preformatted output.",
    "Use title with collapsible when a raw payload needs a compact reveal header.",
    "Use empty with emptyContent when a raw payload was expected but no data exists.",
    "Pass copyLabel so the copy button has a useful accessible label.",
    "Use customContent only when a highlighter already renders its own pre or structured HTML.",
  ],
  avoid: [
    "Do not use for parsed key-value data or tables.",
    "Do not put tab state, decoding, or syntax highlighting setup inside RawDataBlock.",
    "Do not write local pre/code/copy button styles when RawDataBlock fits.",
  ],
  sections: [
    {
      id: "raw-data-block-embedded-tabs",
      title: "Embedded In Tabs",
      description:
        "Large raw data inside the same ContentTabs shell used for code and storage views.",
      content: <EmbeddedTabsSample />,
    },
    {
      id: "raw-data-block-standalone",
      title: "Standalone",
      description: "Framed raw payloads with copy behavior, fixed height, and wrap control.",
      content: <StandaloneSamples />,
    },
    {
      id: "raw-data-block-collapsible",
      title: "Collapsible Title",
      description:
        "A header row can own reveal state while copy remains available from the header.",
      content: <CollapsibleSamples />,
    },
    {
      id: "raw-data-block-empty",
      title: "Empty Data",
      description: "A missing raw payload gets a quiet empty state instead of fake raw text.",
      content: <EmptySamples />,
    },
  ],
} satisfies ComponentGallery
