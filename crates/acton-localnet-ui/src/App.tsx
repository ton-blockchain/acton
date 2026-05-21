import * as React from "react"
import {useEffect, useMemo, useState} from "react"
import {BrowserRouter, Navigate, Route, Routes, useLocation, useNavigate} from "react-router-dom"
import {ToastProvider} from "@acton/shared-ui"

import {Moon, Sun} from "lucide-react"

import {TonClient} from "./explorer/api/client"
import {hashToHex, toTestnetAddress} from "./explorer/components/utils"
import {AddressBookProvider} from "./explorer/hooks/useAddressBook"
import {DashboardPage} from "./dashboard/DashboardPage"
import {AccountPage} from "./explorer/pages/AccountPage"
import {ExplorerIndexPage} from "./explorer/pages/ExplorerIndexPage"
import {NftsPage} from "./explorer/pages/NftsPage"
import {TokensPage} from "./explorer/pages/TokensPage"
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
  const navigate = useNavigate()
  const location = useLocation()
  const isDashboardRoute = location.pathname.startsWith("/dashboard")

  return (
    <div className={styles.app}>
      <header className={styles.header}>
        <div className={styles.headerContent}>
          <div className={styles.logoSection}>
            <button
              type="button"
              className={styles.logo}
              onClick={() => {
                void navigate("/")
              }}
            >
              <svg
                width="20"
                height="20"
                viewBox="0 0 24 24"
                fill="white"
                role="img"
                aria-label="Logo"
              >
                <title>Logo</title>
                <path d="M12 2L2 19h20L12 2zm0 3.8L18.4 17H5.6L12 5.8z" />
              </svg>
            </button>
            <nav className={styles.nav}>
              <button
                type="button"
                className={`${styles.navItem} ${
                  location.pathname.startsWith("/dashboard") ? styles.navItemActive : ""
                }`}
                onClick={() => {
                  void navigate("/dashboard")
                }}
              >
                Dashboard
              </button>
              <button
                type="button"
                className={`${styles.navItem} ${
                  location.pathname.startsWith("/explorer") ? styles.navItemActive : ""
                }`}
                onClick={() => {
                  void navigate("/explorer")
                }}
              >
                Explorer
              </button>
              <button
                type="button"
                className={`${styles.navItem} ${
                  location.pathname.startsWith("/tokens") ? styles.navItemActive : ""
                }`}
                onClick={() => {
                  void navigate("/tokens")
                }}
              >
                Tokens
              </button>
              <button
                type="button"
                className={`${styles.navItem} ${
                  location.pathname.startsWith("/nfts") ? styles.navItemActive : ""
                }`}
                onClick={() => {
                  void navigate("/nfts")
                }}
              >
                NFTs
              </button>
            </nav>
          </div>

          <HeaderSearch />

          {isDashboardRoute ? undefined : (
            <div className={styles.themeSection}>
              <button
                type="button"
                onClick={() => setTheme(theme === "light" ? "dark" : "light")}
                className={styles.themeButton}
                aria-label="Toggle theme"
              >
                <div className={styles.themeIconWrapper}>
                  <Sun
                    className={`${styles.themeIcon} ${theme === "light" ? styles.active : ""}`}
                    size={18}
                  />
                  <Moon
                    className={`${styles.themeIcon} ${theme === "dark" ? styles.active : ""}`}
                    size={18}
                  />
                </div>
              </button>
            </div>
          )}
        </div>
      </header>
      <main className={`${styles.main} ${isDashboardRoute ? styles.mainDashboard : ""}`}>
        <Routes>
          <Route path="/" element={<Navigate to="/dashboard" replace />} />
          <Route path="/dashboard" element={<DashboardPage client={client} theme={theme} setTheme={setTheme} />} />
          <Route path="/dashboard/faucet" element={<DashboardPage client={client} theme={theme} setTheme={setTheme} />} />
          <Route path="/dashboard/tokens" element={<DashboardPage client={client} theme={theme} setTheme={setTheme} />} />
          <Route path="/dashboard/nfts" element={<DashboardPage client={client} theme={theme} setTheme={setTheme} />} />
          <Route path="/explorer" element={<ExplorerIndexPage />} />
          <Route path="/explorer/address/:address" element={<AccountPage client={client} />} />
          <Route path="/tokens" element={<TokensPage client={client} />} />
          <Route path="/nfts" element={<NftsPage client={client} />} />
          <Route path="/explorer/tx/:hash" element={<TransactionPage client={client} />} />
          <Route path="*" element={<Navigate to="/dashboard" replace />} />
        </Routes>
      </main>
    </div>
  )
}

const HeaderSearch: React.FC = () => {
  const navigate = useNavigate()
  return (
    <div className={styles.searchSection}>
      <div className={styles.searchBox}>
        <input
          type="text"
          placeholder="Search by address or hash"
          className={styles.searchInput}
          onKeyDown={e => {
            if (e.key === "Enter") {
              const val = (e.target as HTMLInputElement).value.trim()
              const hashHex = hashToHex(val)
              if (hashHex) {
                void navigate(`/explorer/tx/${hashHex}`)
              } else {
                const formatted = toTestnetAddress(val)
                void navigate(`/explorer/address/${formatted ?? val}`)
              }
            }
          }}
        />
      </div>
    </div>
  )
}
