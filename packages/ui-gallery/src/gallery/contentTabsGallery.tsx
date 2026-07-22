import {ContentTabs, RawDataBlock, type ContentTab} from "@acton/ui"
import {useRef, useState} from "react"

import styles from "./contentTabsGallery.module.css"
import type {ComponentGallery} from "./types"

type CodeTab = "disasm" | "base64" | "hex" | "hex-hash" | "base64-hash"
type TableTab = "messages" | "storage" | "events"
type AsyncTab = "overview" | "raw-body" | "trace-log"

const codeTabs = [
  {value: "disasm", label: "disasm"},
  {value: "base64", label: "base64"},
  {value: "hex", label: "hex"},
  {value: "hex-hash", label: "hex hash"},
  {value: "base64-hash", label: "base64 hash"},
] satisfies readonly ContentTab<CodeTab>[]

const tableTabs = [
  {value: "messages", label: "messages"},
  {value: "storage", label: "storage"},
  {value: "events", label: "events"},
] satisfies readonly ContentTab<TableTab>[]

const asyncTabs = [
  {value: "overview", label: "overview"},
  {value: "raw-body", label: "raw body"},
  {value: "trace-log", label: "trace log"},
] satisfies readonly ContentTab<AsyncTab>[]

const encodedContent = {
  base64: "te6cckEBAQEALwAAWkIAAgE0AwEBAcACAEPKAVgA+KfqUAT6QO1E0NMfifAwJwIDAQGBAQUB",
  hex: "b5ee9c7241010101002f00005a420002013403010101c0020043ca015800f8a7ea5004fa40ed44d0d31f89f0302702030101810105",
  "hex-hash": "65a184650d89a7a435714780a2f6084a8b1c11180c76672cc54f5c6412a23fc0",
  "base64-hash": "ZaGEZQ2Jp6Q1cUeAovYISoscERgMdmcsxU9cZBKiP8A=",
} satisfies Record<Exclude<CodeTab, "disasm">, string>

const disasmContent = [
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
  "            PUSHINT_LONG 1702392942",
  "            NEQ",
  "        }",
  "    }",
].join("\n")

function CodeViewerSample() {
  const [activeTab, setActiveTab] = useState<CodeTab>("disasm")
  const value = activeTab === "disasm" ? disasmContent : encodedContent[activeTab]

  return (
    <ContentTabs
      ariaLabel="Contract code formats"
      tabs={codeTabs}
      value={activeTab}
      onValueChange={setActiveTab}
      panelClassName={styles.codePanel}
    >
      <RawDataBlock
        variant="embedded"
        value={value}
        copyLabel={activeTab}
        wrap={activeTab !== "disasm"}
        maxHeight="32rem"
      >
        {activeTab === "disasm" ? <DisasmPreview /> : undefined}
      </RawDataBlock>
    </ContentTabs>
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

function TableContentSample() {
  const [activeTab, setActiveTab] = useState<TableTab>("messages")

  return (
    <ContentTabs
      ariaLabel="Trace content groups"
      tabs={tableTabs}
      value={activeTab}
      onValueChange={setActiveTab}
      panelClassName={styles.tablePanel}
    >
      <table className={styles.table}>
        <thead>
          <tr>
            <th>name</th>
            <th>value</th>
            <th>status</th>
          </tr>
        </thead>
        <tbody>
          {getTableRows(activeTab).map(row => (
            <tr key={row.name}>
              <td>{row.name}</td>
              <td>{row.value}</td>
              <td>{row.status}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </ContentTabs>
  )
}

function PromiseLoadingSample() {
  const requestIdRef = useRef(0)
  const [activeTab, setActiveTab] = useState<AsyncTab>("overview")
  const [loadedTabs, setLoadedTabs] = useState<ReadonlySet<AsyncTab>>(
    () => new Set<AsyncTab>(["overview"]),
  )

  const handleValueChange = (nextTab: AsyncTab) => {
    if (loadedTabs.has(nextTab)) {
      setActiveTab(nextTab)
      return
    }

    const requestId = requestIdRef.current + 1
    requestIdRef.current = requestId

    return new Promise<void>(resolve => {
      globalThis.setTimeout(() => {
        if (requestId === requestIdRef.current) {
          setLoadedTabs(current => new Set(current).add(nextTab))
          setActiveTab(nextTab)
        }
        resolve()
      }, 900)
    })
  }

  return (
    <ContentTabs
      ariaLabel="Async tab content"
      tabs={asyncTabs}
      value={activeTab}
      onValueChange={handleValueChange}
      panelClassName={styles.asyncPanel}
      loadingLabel="Loading tab content"
    >
      <div className={styles.asyncContent}>
        <h4>{getAsyncTitle(activeTab)}</h4>
        <p>{getAsyncDescription(activeTab)}</p>
      </div>
    </ContentTabs>
  )
}

function getTableRows(tab: TableTab) {
  if (tab === "storage") {
    return [
      {name: "walletId", value: "2 147 483 645", status: "decoded"},
      {name: "seqno", value: "0", status: "decoded"},
      {name: "publicKey", value: "e3f4...9a21", status: "available"},
    ]
  }

  if (tab === "events") {
    return [
      {name: "deploy", value: "lt 48169205000001", status: "confirmed"},
      {name: "ticktock", value: "skipped", status: "inactive"},
      {name: "bounce", value: "none", status: "clean"},
    ]
  }

  return [
    {name: "external-in", value: "wallet-v5", status: "accepted"},
    {name: "internal", value: "sale-v4", status: "sent"},
    {name: "excess", value: "0.031 TON", status: "refunded"},
  ]
}

function getAsyncTitle(tab: AsyncTab) {
  if (tab === "raw-body") return "Raw Body"
  if (tab === "trace-log") return "Trace Log"
  return "Overview"
}

function getAsyncDescription(tab: AsyncTab) {
  if (tab === "raw-body") {
    return "Loaded payload body with decoded opcode, sender, and raw cell references."
  }

  if (tab === "trace-log") {
    return "Loaded VM trace log grouped by compute phase and emitted actions."
  }

  return "Already available summary content for the selected transaction."
}

export const contentTabsGallery = {
  id: "content-tabs",
  title: "ContentTabs",
  status: "ready",
  summary:
    "ContentTabs renders compact connected tabs above a bordered content panel for code viewers, encoded data, tables, and other switchable technical content.",
  importStatement: 'import { ContentTabs } from "@acton/ui"',
  agentSummary:
    "Use ContentTabs for tabbed content panels. Keep the panel content external through children; do not bake code/table rendering into the tabs.",
  usage: [
    "Use for disasm/base64/hex viewers and other compact technical content groups.",
    "Use controlled state so app code owns the selected tab and the rendered content.",
    "Return a Promise from onValueChange when switching tabs requires async loading.",
    "Pass panelClassName when the panel needs code scrolling, table layout, or custom padding.",
  ],
  avoid: [
    "Do not put domain decoding logic inside ContentTabs.",
    "Do not create one-off tab button styles in feature code when this connected panel pattern fits.",
    "Do not use when tabs do not share the same framed content area.",
  ],
  sections: [
    {
      id: "content-tabs-code-viewer",
      title: "Code Viewer",
      description: "Connected tabs above a scrollable panel for disasm and encoded data formats.",
      content: <CodeViewerSample />,
    },
    {
      id: "content-tabs-table-content",
      title: "Table Content",
      description: "The same tabs shell with arbitrary content inside the panel.",
      content: <TableContentSample />,
    },
    {
      id: "content-tabs-promise-loading",
      title: "Promise Loading",
      description:
        "A tab switch that shows the default Skeleton fallback until async content resolves.",
      content: <PromiseLoadingSample />,
    },
  ],
} satisfies ComponentGallery
