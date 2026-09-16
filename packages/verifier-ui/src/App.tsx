import {lazy, Suspense, useEffect} from "react"
import {Route, Routes, useLocation, useNavigate} from "react-router"

import {AppShell} from "./components/AppShell"
import {SearchBox} from "./components/SearchBox"
import {createVerifierApi} from "./lib/api"
import {getPathLookupValue, lookupPath} from "./lib/target"
import {
  readVerifiedContractsPage,
  verifiedContractsPageSearch,
} from "./lib/verified-contracts-pagination"
import {HomePage} from "./pages/HomePage"

const StatisticsPage = lazy(async () => {
  const module = await import("./pages/StatisticsPage")
  return {default: module.StatisticsPage}
})
const VerifiedContractPage = lazy(async () => {
  const module = await import("./pages/VerifiedContractPage")
  return {default: module.VerifiedContractPage}
})
const VerifiedContractsPage = lazy(async () => {
  const module = await import("./pages/VerifiedContractsPage")
  return {default: module.VerifiedContractsPage}
})

const api = createVerifierApi()

function pageTitle(pathname: string): string {
  const normalizedPath = pathname.replace(/\/+$/, "") || "/"
  if (normalizedPath === "/") return "TON Verifier"
  if (normalizedPath === "/statistics") return "Statistics | TON Verifier"
  if (normalizedPath === "/verified") return "Verified Contracts | TON Verifier"
  return "Verified Contract | TON Verifier"
}

function ContractRoute() {
  const location = useLocation()
  const navigate = useNavigate()
  const target = getPathLookupValue()
  const selectedSourcePath = new URLSearchParams(location.search).get("file") ?? undefined

  return (
    <AppShell headerAccessory={<SearchBox key={target} initialValue={target} variant="header" />}>
      <Suspense fallback={null}>
        <VerifiedContractPage
          api={api}
          target={target}
          selectedSourcePath={selectedSourcePath}
          onSelectedSourcePathChange={path => {
            const params = new URLSearchParams(location.search)
            params.set("file", path)
            void navigate(`${location.pathname}?${params.toString()}`, {replace: true})
          }}
        />
      </Suspense>
    </AppShell>
  )
}

function StatisticsRoute() {
  return (
    <AppShell>
      <Suspense fallback={null}>
        <StatisticsPage api={api} />
      </Suspense>
    </AppShell>
  )
}

function VerifiedContractsRoute() {
  const location = useLocation()
  const navigate = useNavigate()
  const page = readVerifiedContractsPage(location.search)

  return (
    <AppShell>
      <Suspense fallback={null}>
        <VerifiedContractsPage
          api={api}
          getContractHref={item => lookupPath(item.code_hash)}
          onOpenContract={item => {
            void navigate(lookupPath(item.code_hash))
          }}
          page={page}
          onPageChange={nextPage => {
            const search = verifiedContractsPageSearch(location.search, nextPage)
            if (search !== location.search.slice(1)) {
              void navigate(
                {
                  pathname: location.pathname,
                  search: search ? `?${search}` : "",
                  hash: location.hash,
                },
                {replace: true},
              )
            }
          }}
          statisticsHref="/statistics"
          onOpenStatistics={() => {
            void navigate("/statistics")
          }}
        />
      </Suspense>
    </AppShell>
  )
}

export function App() {
  const {pathname} = useLocation()

  useEffect(() => {
    document.title = pageTitle(pathname)
  }, [pathname])

  return (
    <Routes>
      <Route path="/" element={<HomePage />} />
      <Route path="/statistics" element={<StatisticsRoute />} />
      <Route path="/verified" element={<VerifiedContractsRoute />} />
      <Route path="*" element={<ContractRoute />} />
    </Routes>
  )
}
