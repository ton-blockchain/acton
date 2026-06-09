import * as React from "react"
import {useEffect, useMemo, useState} from "react"
import {BrowserRouter, Navigate, Route, Routes} from "react-router-dom"
import {ToastProvider} from "@acton/shared-ui"
import type {ThemeMode} from "@acton/shared-ui"

import {TonClient} from "./explorer/api/client"
import {NetworkInfoProvider} from "./explorer/hooks/NetworkInfoProvider"
import {AddressBookProvider} from "./explorer/hooks/useAddressBook"
import {DashboardPage} from "./dashboard/DashboardPage"
import "@acton/shared-ui/styles/tokens.css"
import "./index.css"
import styles from "./App.module.css"

const HOST = (import.meta.env.VITE_LOCALNET_HOST || "").replace(/\/$/, "")
const TONCENTER_API_KEY = import.meta.env.VITE_LOCALNET_TONCENTER_API_KEY?.trim() || undefined
let clientSingleton: TonClient | undefined

const readInitialTheme = (): ThemeMode => {
  const storedTheme = localStorage.getItem("theme")
  if (storedTheme === "dark" || storedTheme === "light") {
    return storedTheme
  }

  return globalThis.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light"
}

function getTonClient(): TonClient {
  clientSingleton ??= new TonClient({
    v2BaseUrl: `${HOST}/api/v2`,
    v3BaseUrl: `${HOST}/api/v3`,
    addressNameBaseUrl: HOST,
    toncenterApiKey: TONCENTER_API_KEY,
  })
  return clientSingleton
}

const ApiReferencePage = React.lazy(async () => {
  const module = await import("./dashboard/pages/ApiReferencePage")
  return {default: module.ApiReferencePage}
})
const FaucetPage = React.lazy(async () => {
  const module = await import("./dashboard/pages/FaucetPage")
  return {default: module.FaucetPage}
})
const HomePage = React.lazy(async () => {
  const module = await import("./dashboard/pages/HomePage")
  return {default: module.HomePage}
})
const ApiCallsPage = React.lazy(async () => {
  const module = await import("./dashboard/pages/ApiCallsPage")
  return {default: module.ApiCallsPage}
})
const NftsPage = React.lazy(async () => {
  const module = await import("./dashboard/pages/NftsPage")
  return {default: module.NftsPage}
})
const TokensPage = React.lazy(async () => {
  const module = await import("./dashboard/pages/TokensPage")
  return {default: module.TokensPage}
})
const WalletsPage = React.lazy(async () => {
  const module = await import("./dashboard/pages/WalletsPage")
  return {default: module.WalletsPage}
})
const AccountPage = React.lazy(async () => {
  const module = await import("./explorer/pages/AccountPage")
  return {default: module.AccountPage}
})
const ExplorerIndexPage = React.lazy(async () => {
  const module = await import("./explorer/pages/ExplorerIndexPage")
  return {default: module.ExplorerIndexPage}
})
const TransactionPage = React.lazy(async () => {
  const module = await import("./explorer/pages/TransactionPage")
  return {default: module.TransactionPage}
})

export const App: React.FC = () => {
  const [theme, setTheme] = useState<ThemeMode>(readInitialTheme)

  const client = useMemo(getTonClient, [])

  useEffect(() => {
    document.documentElement.classList.toggle("dark-theme", theme === "dark")
    document.body.classList.toggle("dark-mode", theme === "dark")
    document.body.classList.toggle("light-mode", theme !== "dark")
    localStorage.setItem("theme", theme)
  }, [theme])

  return (
    <BrowserRouter>
      <ToastProvider>
        <NetworkInfoProvider client={client}>
          <AddressBookProvider client={client}>
            <AppContent client={client} theme={theme} setTheme={setTheme} />
          </AddressBookProvider>
        </NetworkInfoProvider>
      </ToastProvider>
    </BrowserRouter>
  )
}

interface AppContentProps {
  readonly client: TonClient
  readonly theme: ThemeMode
  readonly setTheme: (theme: ThemeMode) => void
}

const AppContent: React.FC<AppContentProps> = ({client, theme, setTheme}) => {
  return (
    <div className={styles.app}>
      <main className={styles.main}>
        <Routes>
          <Route path="/" element={<Navigate to="/dashboard" replace />} />
          <Route
            path="/dashboard"
            element={
              <DashboardPage client={client} theme={theme} setTheme={setTheme}>
                <RouteSuspense>
                  <HomePage client={client} />
                </RouteSuspense>
              </DashboardPage>
            }
          />
          <Route
            path="/faucet"
            element={
              <DashboardPage client={client} theme={theme} setTheme={setTheme}>
                <RouteSuspense>
                  <FaucetPage client={client} />
                </RouteSuspense>
              </DashboardPage>
            }
          />
          <Route
            path="/wallets"
            element={
              <DashboardPage client={client} theme={theme} setTheme={setTheme}>
                <RouteSuspense>
                  <WalletsPage client={client} host={HOST} />
                </RouteSuspense>
              </DashboardPage>
            }
          />
          <Route
            path="/tokens"
            element={
              <DashboardPage client={client} theme={theme} setTheme={setTheme}>
                <RouteSuspense>
                  <TokensPage client={client} />
                </RouteSuspense>
              </DashboardPage>
            }
          />
          <Route
            path="/nfts"
            element={
              <DashboardPage client={client} theme={theme} setTheme={setTheme}>
                <RouteSuspense>
                  <NftsPage client={client} />
                </RouteSuspense>
              </DashboardPage>
            }
          />
          <Route path="/api-reference" element={<Navigate to="/api-reference/v2" replace />} />
          <Route
            path="/api-reference/v2"
            element={
              <DashboardPage client={client} theme={theme} setTheme={setTheme} embedded>
                <RouteSuspense>
                  <ApiReferencePage
                    apiBaseUrl={`${HOST}/api/v2`}
                    theme={theme}
                    toncenterApiKey={TONCENTER_API_KEY}
                    version="v2"
                  />
                </RouteSuspense>
              </DashboardPage>
            }
          />
          <Route
            path="/api-reference/v3"
            element={
              <DashboardPage client={client} theme={theme} setTheme={setTheme} embedded>
                <RouteSuspense>
                  <ApiReferencePage
                    apiBaseUrl={`${HOST}/api/v3`}
                    theme={theme}
                    toncenterApiKey={TONCENTER_API_KEY}
                    version="v3"
                  />
                </RouteSuspense>
              </DashboardPage>
            }
          />
          <Route
            path="/api-reference/control"
            element={
              <DashboardPage client={client} theme={theme} setTheme={setTheme} embedded>
                <RouteSuspense>
                  <ApiReferencePage apiBaseUrl={HOST} theme={theme} version="control" />
                </RouteSuspense>
              </DashboardPage>
            }
          />
          <Route path="/dashboard/faucet" element={<Navigate to="/faucet" replace />} />
          <Route path="/dashboard/wallets" element={<Navigate to="/wallets" replace />} />
          <Route path="/dashboard/tokens" element={<Navigate to="/tokens" replace />} />
          <Route path="/dashboard/nfts" element={<Navigate to="/nfts" replace />} />
          <Route path="/dashboard/json-rpc-calls" element={<Navigate to="/api-calls" replace />} />
          <Route path="/dashboard/api-calls" element={<Navigate to="/api-calls" replace />} />
          <Route path="/json-rpc-calls" element={<Navigate to="/api-calls" replace />} />
          <Route
            path="/api-calls"
            element={
              <DashboardPage client={client} theme={theme} setTheme={setTheme}>
                <RouteSuspense>
                  <ApiCallsPage client={client} />
                </RouteSuspense>
              </DashboardPage>
            }
          />
          <Route
            path="/explorer"
            element={
              <DashboardPage client={client} theme={theme} setTheme={setTheme} embedded>
                <RouteSuspense>
                  <ExplorerIndexPage />
                </RouteSuspense>
              </DashboardPage>
            }
          />
          <Route
            path="/explorer/address/:address"
            element={
              <DashboardPage client={client} theme={theme} setTheme={setTheme} embedded>
                <RouteSuspense>
                  <AccountPage client={client} />
                </RouteSuspense>
              </DashboardPage>
            }
          />
          <Route
            path="/explorer/tx/:hash"
            element={
              <DashboardPage client={client} theme={theme} setTheme={setTheme} embedded>
                <RouteSuspense>
                  <TransactionPage client={client} />
                </RouteSuspense>
              </DashboardPage>
            }
          />
          <Route path="*" element={<Navigate to="/dashboard" replace />} />
        </Routes>
      </main>
    </div>
  )
}

const RouteSuspense: React.FC<{readonly children: React.ReactNode}> = ({children}) => (
  <React.Suspense fallback={<div className={styles.routeLoading}>Loading…</div>}>
    {children}
  </React.Suspense>
)
