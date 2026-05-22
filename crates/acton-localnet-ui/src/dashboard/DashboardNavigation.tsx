import {
  ArrowUpRight,
  BookOpen,
  Boxes,
  Github,
  Image,
  KeyRound,
  LayoutGrid,
  Moon,
  Search as SearchIcon,
  Sun,
  Wallet,
} from "lucide-react"
import type {LucideIcon} from "lucide-react"
import * as React from "react"
import {useLocation, useNavigate} from "react-router-dom"

import type {TonClient} from "../explorer/api/client"
import {readExplorerLastPath, writeExplorerLastPath} from "../explorer/explorerResume"

import {DashboardSearch} from "./DashboardSearch"
import styles from "./DashboardPage.module.css"

interface DashboardNavigationProps {
  readonly client: TonClient
  readonly theme: string
  readonly setTheme: (theme: string) => void
}

interface SidebarItem {
  readonly label: string
  readonly icon: LucideIcon
  readonly path?: string
  readonly href?: string
}

const mainItems: SidebarItem[] = [
  {label: "Home", icon: LayoutGrid, path: "/dashboard"},
  {label: "Explorer", icon: SearchIcon, path: "/explorer"},
  {label: "Wallets", icon: KeyRound, path: "/wallets"},
  {label: "Faucet", icon: Wallet, path: "/faucet"},
  {label: "Tokens", icon: Boxes, path: "/tokens"},
  {label: "NFTs", icon: Image, path: "/nfts"},
]

const footerItems: SidebarItem[] = [
  {
    label: "Documentation",
    icon: BookOpen,
    href: "https://ton-blockchain.github.io/acton/docs/welcome",
  },
  {label: "GitHub", icon: Github, href: "https://github.com/ton-blockchain/acton"},
]

export const DashboardNavigation: React.FC<DashboardNavigationProps> = ({
  client,
  theme,
  setTheme,
}) => {
  const location = useLocation()
  const navigate = useNavigate()
  const [explorerPath, setExplorerPath] = React.useState(() => readExplorerLastPath())

  React.useEffect(() => {
    if (!location.pathname.startsWith("/explorer")) {
      return
    }

    const nextPath = `${location.pathname}${location.search}${location.hash}`
    writeExplorerLastPath(nextPath)
    setExplorerPath(nextPath)
  }, [location.hash, location.pathname, location.search])

  return (
    <aside className={styles.sidebar}>
      <div className={styles.sidebarHeader}>
        <div className={styles.workspaceHeader}>
          <span className={styles.workspaceMark} />
          <span className={styles.workspaceBody}>
            <span className={styles.workspaceName}>TON Localnet</span>
            <span className={styles.workspaceMeta}>by Acton</span>
          </span>
        </div>
      </div>

      <div className={styles.topControls}>
        <DashboardSearch client={client} />
      </div>

      <div className={styles.navScroll}>
        <nav className={styles.nav}>
          <div className={styles.navSection}>
            {mainItems.map(item => {
              const Icon = item.icon
              const targetPath = item.path === "/explorer" ? explorerPath : item.path
              const isActive =
                item.path === "/explorer"
                  ? location.pathname.startsWith("/explorer")
                  : item.path === location.pathname

              return (
                <button
                  type="button"
                  key={item.label}
                  className={`${styles.navItem} ${isActive ? styles.navItemActive : ""}`}
                  onClick={() => {
                    if (targetPath) {
                      void navigate(targetPath)
                    }
                  }}
                >
                  <span className={styles.navItemMain}>
                    <Icon size={18} />
                    <span>{item.label}</span>
                  </span>
                </button>
              )
            })}
          </div>

          <div className={styles.navDivider} />

          <div className={styles.navFooter}>
            <div className={styles.navSection}>
              {footerItems.map(item => {
                const Icon = item.icon

                return (
                  <a
                    key={item.label}
                    className={styles.navItem}
                    href={item.href}
                    target="_blank"
                    rel="noreferrer"
                  >
                    <span className={styles.navItemMain}>
                      <Icon size={18} />
                      <span>{item.label}</span>
                    </span>
                    <ArrowUpRight size={14} />
                  </a>
                )
              })}
            </div>

            <button
              type="button"
              className={styles.themeSwitch}
              aria-label="Toggle Theme"
              data-theme-toggle=""
              onClick={() => setTheme(theme === "light" ? "dark" : "light")}
            >
              <Sun
                fill="currentColor"
                className={`${styles.themeSwitchItem} ${theme === "light" ? styles.themeSwitchItemActive : ""}`}
              />
              <Moon
                fill="currentColor"
                className={`${styles.themeSwitchItem} ${theme === "dark" ? styles.themeSwitchItemActive : ""}`}
              />
            </button>
          </div>
        </nav>
      </div>
    </aside>
  )
}
