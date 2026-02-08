import { Address } from "@ton/core"
import type React from "react"
import { useEffect, useMemo, useState } from "react"
import { BrowserRouter, Navigate, Route, Routes, useNavigate, useParams } from "react-router-dom"
import { TonClient } from "./explorer/api/client"
import { ExplorerIndexPage } from "./explorer/pages/ExplorerIndexPage"
import { TransactionPage } from "./explorer/pages/TransactionPage"
import { TonExplorer } from "./explorer/TonExplorer"
import "@acton/shared-ui/styles/tokens.css"
import "./index.css"
import { Moon, Sun } from "lucide-react"
import styles from "./App.module.css"

export const App: React.FC = () => {
  const [theme, setTheme] = useState(() => {
    return (
      localStorage.getItem("theme") ||
      (window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light")
    )
  })

  const client = useMemo(() => new TonClient("http://localhost:3010/api"), [])

  useEffect(() => {
    import("./explorer/components/utils").then((utils) => {
      utils.setTonClientInstance(client)
    })
  }, [client])

  useEffect(() => {
    document.documentElement.classList.toggle("dark-theme", theme === "dark")
    localStorage.setItem("theme", theme)
  }, [theme])

  return (
    <BrowserRouter>
      <div className={styles.app}>
        <header className={styles.header}>
          <div className={styles.headerContent}>
            <div className={styles.logoSection}>
              <div className={styles.logo} onClick={() => (window.location.href = "/")}>
                <svg width="20" height="20" viewBox="0 0 24 24" fill="white">
                  <path d="M12 2L2 19h20L12 2zm0 3.8L18.4 17H5.6L12 5.8z" />
                </svg>
              </div>
              <nav className={styles.nav}>
                <div className={`${styles.navItem} ${styles.navItemActive}`}>Explorer</div>
                <div className={styles.navItem}>TOKENS</div>
              </nav>
            </div>

            <HeaderSearch />

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
          </div>
        </header>
        <main className={styles.main}>
          <Routes>
            <Route path="/" element={<Navigate to="/explorer" replace />} />
            <Route path="/explorer" element={<ExplorerIndexPage />} />
            <Route
              path="/explorer/address/:address"
              element={<TonExplorerWrapper client={client} />}
            />
            <Route path="/tx/:hash" element={<TransactionPage client={client} />} />
            <Route path="*" element={<Navigate to="/explorer" replace />} />
          </Routes>
        </main>
      </div>
    </BrowserRouter>
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
          onKeyDown={(e) => {
            if (e.key === "Enter") {
              const val = (e.target as HTMLInputElement).value
              if (val.length === 64) {
                navigate(`/tx/${val}`)
              } else {
                try {
                  const formatted = Address.parse(val).toString({ testOnly: true })
                  navigate(`/explorer/address/${formatted}`)
                } catch {
                  navigate(`/explorer/address/${val}`)
                }
              }
            }
          }}
        />
      </div>
    </div>
  )
}

const TonExplorerWrapper: React.FC<{ client: TonClient }> = ({ client }) => {
  const { address } = useParams<{ address: string }>()
  const navigate = useNavigate()

  const handleSearch = (addr: string) => {
    let finalAddr = addr
    try {
      if (addr) {
        finalAddr = Address.parse(addr).toString({ testOnly: true })
      }
    } catch {
      // Keep original if not a valid TON address
    }

    if (finalAddr) {
      navigate(`/explorer/address/${finalAddr}`)
    } else {
      navigate("/explorer")
    }
  }

  return (
    <TonExplorer client={client} externalAddress={address || ""} onAddressChange={handleSearch} />
  )
}
