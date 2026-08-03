import {createRoot} from "react-dom/client"
import {useState} from "react"

import {AppShell} from "../components/AppShell"
import {createVerifierApi} from "../lib/api"
import {lookupPath} from "../lib/target"
import {
  readVerifiedContractsPage,
  verifiedContractsPageSearch,
} from "../lib/verified-contracts-pagination"
import {VerifiedContractsPage} from "../pages/VerifiedContractsPage"
import "../global.css"

const api = createVerifierApi()

function VerifiedContractsEntry() {
  const [page, setPage] = useState(() => readVerifiedContractsPage(globalThis.location.search))

  return (
    <AppShell>
      <VerifiedContractsPage
        api={api}
        getContractHref={item => lookupPath(item.code_hash)}
        onOpenContract={item => globalThis.location.assign(lookupPath(item.code_hash))}
        page={page}
        onPageChange={nextPage => {
          const search = verifiedContractsPageSearch(globalThis.location.search, nextPage)
          const nextUrl = `${globalThis.location.pathname}${search ? `?${search}` : ""}${
            globalThis.location.hash
          }`

          globalThis.history.replaceState(globalThis.history.state, "", nextUrl)
          setPage(nextPage)
        }}
        statisticsHref="/statistics"
      />
    </AppShell>
  )
}

const root = document.getElementById("root")
if (!root) throw new Error("Verifier root element was not found")
createRoot(root).render(<VerifiedContractsEntry />)
