import {createRoot} from "react-dom/client"

import {AppShell} from "../components/AppShell"
import {SearchBox} from "../components/SearchBox"
import {createVerifierApi} from "../lib/api"
import {getPathLookupValue} from "../lib/target"
import {VerifiedContractPage} from "../pages/VerifiedContractPage"
import "../global.css"

const api = createVerifierApi()

function ContractEntry() {
  const target = getPathLookupValue()
  const selectedSourcePath =
    new URLSearchParams(globalThis.location.search).get("file") ?? undefined

  return (
    <AppShell headerAccessory={<SearchBox initialValue={target} variant="header" />}>
      <VerifiedContractPage
        api={api}
        target={target}
        selectedSourcePath={selectedSourcePath}
        onSelectedSourcePathChange={path => {
          const url = new URL(globalThis.location.href)
          url.searchParams.set("file", path)
          globalThis.history.replaceState(
            globalThis.history.state,
            "",
            `${url.pathname}${url.search}${url.hash}`,
          )
        }}
      />
    </AppShell>
  )
}

const root = document.getElementById("root")
if (!root) throw new Error("Verifier root element was not found")
createRoot(root).render(<ContractEntry />)
