import {ThemeSwitch, ToastProvider} from "@acton/shared-ui"
import {Github} from "lucide-react"
import {useEffect, useMemo, useState} from "react"
import type {FC} from "react"
import {BrowserRouter, Link, Navigate, Route, Routes} from "react-router-dom"

import {TonClient} from "../../acton-localnet-ui/src/explorer/api/client"
import {getBundledCompilerAbis} from "../../acton-localnet-ui/src/explorer/api/compilerAbiCatalog"
import {AddressBookProvider} from "../../acton-localnet-ui/src/explorer/hooks/useAddressBook"
import {ExplorerRoutesProvider} from "../../acton-localnet-ui/src/explorer/hooks/useExplorerRoutes"
import {StaticNetworkInfoProvider} from "../../acton-localnet-ui/src/explorer/hooks/StaticNetworkInfoProvider"
import {BlockDetailsPage, BlocksPage} from "../../acton-localnet-ui/src/explorer/pages/BlocksPage"
import {AccountPage} from "../../acton-localnet-ui/src/explorer/pages/AccountPage"
import {ExplorerIndexPage} from "../../acton-localnet-ui/src/explorer/pages/ExplorerIndexPage"
import {TransactionPage} from "../../acton-localnet-ui/src/explorer/pages/TransactionPage"
import type {ThemeMode} from "@acton/shared-ui"
import "@acton/shared-ui/styles/tokens.css"
import "../../acton-localnet-ui/src/index.css"
import styles from "./ExplorerApp.module.css"

const TONCENTER_API_V2_URL =
  import.meta.env.VITE_EXPLORER_TONCENTER_API_V2_URL?.trim().replace(/\/$/, "") ||
  "https://toncenter.com/api/v2"
const TONCENTER_API_V3_URL =
  import.meta.env.VITE_EXPLORER_TONCENTER_API_V3_URL?.trim().replace(/\/$/, "") ||
  "https://toncenter.com/api/v3"
const TONCENTER_API_KEY = import.meta.env.VITE_EXPLORER_TONCENTER_API_KEY?.trim() || undefined

const readInitialTheme = (): ThemeMode => {
  const storedTheme = localStorage.getItem("explorerTheme")
  if (storedTheme === "dark" || storedTheme === "light") {
    return storedTheme
  }

  return globalThis.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light"
}

export const ExplorerApp: FC = () => {
  const [theme, setTheme] = useState<ThemeMode>(readInitialTheme)
  const client = useMemo(
    () =>
      new TonClient({
        v2BaseUrl: TONCENTER_API_V2_URL,
        v3BaseUrl: TONCENTER_API_V3_URL,
        addressNameBaseUrl: "",
        localnetControlEnabled: false,
        toncenterApiKey: TONCENTER_API_KEY,
        compilerAbiLoader: getBundledCompilerAbis,
      }),
    [],
  )

  useEffect(() => {
    document.documentElement.classList.toggle("dark-theme", theme === "dark")
    document.body.classList.toggle("dark-mode", theme === "dark")
    document.body.classList.toggle("light-mode", theme !== "dark")
    localStorage.setItem("explorerTheme", theme)
  }, [theme])

  const toggleTheme = () => setTheme(current => (current === "dark" ? "light" : "dark"))

  return (
    <BrowserRouter>
      <ToastProvider>
        <StaticNetworkInfoProvider>
          <ExplorerRoutesProvider basePath="">
            <AddressBookProvider>
              <div className={styles.appShell}>
                <header className={styles.header}>
                  <div className={styles.headerInner}>
                    <div className={styles.headerPrimary}>
                      <Link className={styles.brand} to="/">
                        <span className={styles.brandText}>actonscan</span>
                      </Link>
                      <nav className={styles.nav} aria-label="Explorer navigation">
                        <Link className={styles.navLink} to="/">
                          Explore
                        </Link>
                        <Link className={styles.navLink} to="/blocks">
                          Blocks
                        </Link>
                      </nav>
                    </div>
                    <div className={styles.headerActions}>
                      <ThemeSwitch
                        theme={theme}
                        onToggleTheme={toggleTheme}
                        aria-label={theme === "dark" ? "Use light theme" : "Use dark theme"}
                      />
                      <a
                        className={styles.githubButton}
                        href="https://github.com/ton-blockchain/acton"
                        target="_blank"
                        rel="noreferrer"
                        title="Open GitHub"
                        aria-label="Open GitHub"
                      >
                        <Github size={18} />
                      </a>
                    </div>
                  </div>
                </header>
                <main className={styles.main}>
                  <Routes>
                    <Route path="/" element={<ExplorerIndexPage />} />
                    <Route path="/blocks" element={<BlocksPage client={client} />} />
                    <Route
                      path="/block/:workchain/:shard/:seqno"
                      element={<BlockDetailsPage client={client} />}
                    />
                    <Route path="/address/:address" element={<AccountPage client={client} />} />
                    <Route path="/tx/:hash" element={<TransactionPage client={client} />} />
                    <Route path="*" element={<Navigate to="/" replace />} />
                  </Routes>
                </main>
              </div>
            </AddressBookProvider>
          </ExplorerRoutesProvider>
        </StaticNetworkInfoProvider>
      </ToastProvider>
    </BrowserRouter>
  )
}
