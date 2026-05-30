import * as React from "react"
import {useEffect, useMemo, useState} from "react"
import {BrowserRouter, Navigate, Route, Routes} from "react-router-dom"
import {ToastProvider} from "@acton/shared-ui"

import {TonClient} from "./explorer/api/client"
import {NetworkInfoProvider} from "./explorer/hooks/NetworkInfoProvider"
import {AddressBookProvider} from "./explorer/hooks/useAddressBook"
import {DashboardPage} from "./dashboard/DashboardPage"
import {FaucetPage} from "./dashboard/pages/FaucetPage"
import {HomePage} from "./dashboard/pages/HomePage"
import {NftsPage} from "./dashboard/pages/NftsPage"
import {TokensPage} from "./dashboard/pages/TokensPage"
import {WalletsPage} from "./dashboard/pages/WalletsPage"
import {AccountPage} from "./explorer/pages/AccountPage"
import {ExplorerIndexPage} from "./explorer/pages/ExplorerIndexPage"
import {TransactionPage} from "./explorer/pages/TransactionPage"
import "@acton/shared-ui/styles/tokens.css"
import "./index.css"
import styles from "./App.module.css"

const HOST = (import.meta.env.VITE_LOCALNET_HOST || "").replace(/\/$/, "")
const ApiReferencePage = React.lazy(async () => {
  const module = await import("./dashboard/pages/ApiReferencePage")
  return {default: module.ApiReferencePage}
})

export const App: React.FC = () => {
  const [theme, setTheme] = useState(() => {
    return (
      localStorage.getItem("theme") ||
      (globalThis.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light")
    )
  })

  const client = useMemo(
    () =>
      new TonClient({
        v2BaseUrl: `${HOST}/api/v2`,
        v3BaseUrl: `${HOST}/api/v3`,
        addressNameBaseUrl: HOST,
      }),
    [],
  )

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
  readonly theme: string
  readonly setTheme: (theme: string) => void
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
                <HomePage client={client} />
              </DashboardPage>
            }
          />
          <Route
            path="/faucet"
            element={
              <DashboardPage client={client} theme={theme} setTheme={setTheme}>
                <FaucetPage client={client} />
              </DashboardPage>
            }
          />
          <Route
            path="/wallets"
            element={
              <DashboardPage client={client} theme={theme} setTheme={setTheme}>
                <WalletsPage client={client} host={HOST} />
              </DashboardPage>
            }
          />
          <Route
            path="/tokens"
            element={
              <DashboardPage client={client} theme={theme} setTheme={setTheme}>
                <TokensPage client={client} />
              </DashboardPage>
            }
          />
          <Route
            path="/nfts"
            element={
              <DashboardPage client={client} theme={theme} setTheme={setTheme}>
                <NftsPage client={client} />
              </DashboardPage>
            }
          />
          <Route path="/api-reference" element={<Navigate to="/api-reference/v2" replace />} />
          <Route
            path="/api-reference/v2"
            element={
              <DashboardPage client={client} theme={theme} setTheme={setTheme} embedded>
                <React.Suspense fallback={<div className={styles.routeLoading}>Loading…</div>}>
                  <ApiReferencePage apiBaseUrl={`${HOST}/api/v2`} theme={theme} version="v2" />
                </React.Suspense>
              </DashboardPage>
            }
          />
          <Route
            path="/api-reference/v3"
            element={
              <DashboardPage client={client} theme={theme} setTheme={setTheme} embedded>
                <React.Suspense fallback={<div className={styles.routeLoading}>Loading…</div>}>
                  <ApiReferencePage apiBaseUrl={`${HOST}/api/v3`} theme={theme} version="v3" />
                </React.Suspense>
              </DashboardPage>
            }
          />
          <Route
            path="/api-reference/control"
            element={
              <DashboardPage client={client} theme={theme} setTheme={setTheme} embedded>
                <React.Suspense fallback={<div className={styles.routeLoading}>Loading…</div>}>
                  <ApiReferencePage apiBaseUrl={HOST} theme={theme} version="control" />
                </React.Suspense>
              </DashboardPage>
            }
          />
          <Route path="/dashboard/faucet" element={<Navigate to="/faucet" replace />} />
          <Route path="/dashboard/wallets" element={<Navigate to="/wallets" replace />} />
          <Route path="/dashboard/tokens" element={<Navigate to="/tokens" replace />} />
          <Route path="/dashboard/nfts" element={<Navigate to="/nfts" replace />} />
          <Route
            path="/explorer"
            element={
              <DashboardPage client={client} theme={theme} setTheme={setTheme} embedded>
                <ExplorerIndexPage />
              </DashboardPage>
            }
          />
          <Route
            path="/explorer/address/:address"
            element={
              <DashboardPage client={client} theme={theme} setTheme={setTheme} embedded>
                <AccountPage client={client} />
              </DashboardPage>
            }
          />
          <Route
            path="/explorer/tx/:hash"
            element={
              <DashboardPage client={client} theme={theme} setTheme={setTheme} embedded>
                <TransactionPage client={client} />
              </DashboardPage>
            }
          />
          <Route path="*" element={<Navigate to="/dashboard" replace />} />
        </Routes>
      </main>
    </div>
  )
}
