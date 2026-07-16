import {SearchInput} from "@acton/ui"
import type {SearchInputItem} from "@acton/ui"
import {FileCode2, History, Search} from "lucide-react"
import {useMemo, useState} from "react"

import styles from "./searchInputGallery.module.css"
import type {ComponentGallery} from "./types"

const historyValues = [
  "EQD36X...ur8XSS",
  "9f2c7d43...46f861ba",
  "EQD1tC...EFbIYg",
  "EQD3Xa...qagXQw",
  "EQDwXA...svkDcM",
  "EQAKDb...2IsDYp",
  "EQBvNa...DS-ejP",
  "EQAKIO...I9KywO",
] as const

function HistorySample() {
  const [value, setValue] = useState("")
  const [history, setHistory] = useState<readonly string[]>(historyValues)
  const items = useMemo<readonly SearchInputItem[]>(
    () =>
      history.map(item => ({
        id: item,
        label: item,
        icon: <History size={16} />,
        onSelect: () => setValue(item),
        onRemove: () => setHistory(current => current.filter(value => value !== item)),
        removeLabel: `Remove ${item} from history`,
      })),
    [history],
  )

  return (
    <div className={styles.historySample}>
      <SearchInput
        ariaLabel="Search history example"
        items={items}
        onValueChange={setValue}
        open
        placeholder="Search by address or hash"
        value={value}
      />
    </div>
  )
}

function ResultsSample() {
  const [value, setValue] = useState("jetton")
  const items: readonly SearchInputItem[] = [
    {
      id: "asset",
      label: "Jetton Minter",
      description: "EQDwXA...svkDcM",
      icon: <Search size={16} />,
      onSelect: () => setValue("Jetton Minter"),
    },
    {
      id: "abi",
      label: "InternalTransferStep",
      description: "ABI · Declaration · JettonWallet · 0x178d4519",
      icon: <FileCode2 size={16} />,
      onSelect: () => setValue("InternalTransferStep"),
    },
  ]

  return (
    <div className={styles.resultsSample}>
      <SearchInput
        ariaLabel="Compact search results example"
        items={items}
        onValueChange={setValue}
        open
        placeholder="Search by address or hash"
        size="sm"
        value={value}
      />
    </div>
  )
}

function StateSamples() {
  const [value, setValue] = useState("not an address")

  return (
    <div className={styles.states}>
      <SearchInput
        ariaLabel="Empty search example"
        items={[]}
        onValueChange={setValue}
        placeholder="No matches"
        value=""
      />
      <SearchInput
        ariaLabel="Invalid search example"
        invalid
        items={[]}
        onValueChange={setValue}
        placeholder="Search by address or hash"
        value={value}
      />
    </div>
  )
}

export const searchInputGallery = {
  id: "search-input",
  title: "Search Input",
  status: "ready",
  summary:
    "SearchInput combines a search field with removable history and rich result rows while leaving domain lookup to its caller.",
  importStatement: 'import { SearchInput } from "@acton/ui"',
  agentSummary:
    "Use SearchInput for search controls that reveal history or result rows. The caller owns query resolution, persistence, and navigation.",
  usage: [
    "Pass already resolved items with stable ids and selection callbacks.",
    "Use description for a secondary address, kind, or technical identifier.",
    "Use onRemove for persisted history entries; the component owns the remove control and focus behavior.",
    "Return false from onSubmit when validation fails and the control should remain open.",
  ],
  avoid: [
    "Do not fetch data, access localStorage, or navigate from inside SearchInput.",
    "Do not recreate the input, floating list, or delayed blur handling in feature code.",
    "Do not use SearchInput for a static select or command palette.",
  ],
  sections: [
    {
      id: "search-input-history",
      title: "Search History",
      description:
        "One-line entries can be selected or removed without closing before the action runs.",
      content: <HistorySample />,
    },
    {
      id: "search-input-results",
      title: "Rich Results",
      description: "Compact two-line rows support matched entities and ABI declarations.",
      content: <ResultsSample />,
    },
    {
      id: "search-input-states",
      title: "Empty and Invalid",
      description:
        "An empty item list stays closed; invalid state marks the control without inline errors.",
      content: <StateSamples />,
    },
  ],
} satisfies ComponentGallery
