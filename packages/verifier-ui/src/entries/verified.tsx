import {createRoot} from "react-dom/client"

import {AppShell} from "../components/AppShell"
import {createVerifierApi} from "../lib/api"
import {lookupPath} from "../lib/target"
import {VerifiedContractsPage} from "../pages/VerifiedContractsPage"
import "../global.css"

const api = createVerifierApi()

function VerifiedContractsEntry() {
  return (
    <AppShell>
      <VerifiedContractsPage
        api={api}
        getContractHref={item => lookupPath(item.code_hash)}
        onOpenContract={item => globalThis.location.assign(lookupPath(item.code_hash))}
      />
    </AppShell>
  )
}

const root = document.getElementById("root")
if (!root) throw new Error("Verifier root element was not found")
createRoot(root).render(<VerifiedContractsEntry />)
