import * as React from "react"
import {useEffect, useMemo, useState} from "react"
import {BrowserRouter, Navigate, Route, Routes} from "react-router-dom"
import {ToastProvider} from "@acton/shared-ui"

import {TonClient} from "./explorer/api/client"
import {AddressBookProvider} from "./explorer/hooks/useAddressBook"
import {DashboardPage} from "./dashboard/DashboardPage"
import {AccountPage} from "./explorer/pages/AccountPage"
import {ExplorerIndexPage} from "./explorer/pages/ExplorerIndexPage"
import {TransactionPage} from "./explorer/pages/TransactionPage"
import "@acton/shared-ui/styles/tokens.css"
import "./index.css"
import styles from "./App.module.css"

const HOST = (import.meta.env.VITE_LOCALNET_HOST || "").replace(/\/$/, "")

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
    localStorage.setItem("theme", theme)
  }, [theme])

  return (
    <BrowserRouter>
      <ToastProvider>
        <AddressBookProvider client={client}>
          <AppContent client={client} theme={theme} setTheme={setTheme} />
        </AddressBookProvider>
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
          <Route path="/dashboard" element={<DashboardPage client={client} theme={theme} setTheme={setTheme} />} />
          <Route path="/dashboard/faucet" element={<DashboardPage client={client} theme={theme} setTheme={setTheme} />} />
          <Route path="/dashboard/tokens" element={<DashboardPage client={client} theme={theme} setTheme={setTheme} />} />
          <Route path="/dashboard/nfts" element={<DashboardPage client={client} theme={theme} setTheme={setTheme} />} />
          <Route
            path="/explorer"
            element={
              <DashboardPage client={client} theme={theme} setTheme={setTheme}>
                <ExplorerIndexPage />
              </DashboardPage>
            }
          />
          <Route
            path="/explorer/address/:address"
            element={
              <DashboardPage client={client} theme={theme} setTheme={setTheme}>
                <AccountPage client={client} />
              </DashboardPage>
            }
          />
          <Route
            path="/explorer/tx/:hash"
            element={
              <DashboardPage client={client} theme={theme} setTheme={setTheme}>
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
