import type {ThemeMode} from "@acton/shared-ui"
import {ThemeSwitch} from "@acton/shared-ui"
import type {LucideIcon} from "lucide-react"
import {
  Activity,
  ArrowUpRight,
  BookOpen,
  Boxes,
  Brackets,
  Check,
  Coins,
  FileCode2,
  FileJson,
  Github,
  HandCoins,
  Image,
  KeyRound,
  LayoutGrid,
  Menu,
  PanelLeftClose,
  PanelLeftOpen,
  Search as SearchIcon,
  Settings2,
  Star,
  Wallet,
  X,
} from "lucide-react"
import type {FC} from "react"
import {Fragment, useCallback, useEffect, useMemo, useState} from "react"
import {useLocation, useNavigate} from "react-router-dom"

import type {TonClient} from "../explorer/api/client"
import {readExplorerLastPath, writeExplorerLastPath} from "../explorer/explorerResume"
import {useNetworkInfo} from "../explorer/hooks/useNetworkInfo"
import styles from "./DashboardPage.module.css"
import {DashboardSearch} from "./DashboardSearch"

interface DashboardNavigationProps {
  readonly client: TonClient
  readonly localnetApiToken?: string
  readonly onOpenAuthTokenOverlay: () => void
  readonly theme: ThemeMode
  readonly setTheme: (theme: ThemeMode) => void
  readonly onToggleSidebar?: () => void
  readonly isSidebarCollapsed?: boolean
  readonly className?: string
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
  {label: "Blocks", icon: Boxes, path: "/explorer/blocks"},
  {label: "Wallets", icon: Wallet, path: "/wallets"},
  {label: "Faucet", icon: HandCoins, path: "/faucet"},
  {label: "Tokens", icon: Coins, path: "/tokens"},
  {label: "NFTs", icon: Image, path: "/nfts"},
]

const sourceItems: SidebarItem[] = [
  {label: "Sources", icon: FileCode2, path: "/explorer/sources"},
  {label: "ABI", icon: FileJson, path: "/explorer/abi"},
]

const apiItems: SidebarItem[] = [
  {label: "API Calls", icon: Activity, path: "/api-calls"},
  {label: "v2 API", icon: FileJson, path: "/api-reference/v2"},
  {label: "v3 API", icon: Brackets, path: "/api-reference/v3"},
  {label: "Control API", icon: Settings2, path: "/api-reference/control"},
]

const navigationSections: Array<{readonly id: string; readonly items: readonly SidebarItem[]}> = [
  {id: "main", items: mainItems},
  {id: "sources", items: sourceItems},
  {id: "api", items: apiItems},
]

const footerItems: SidebarItem[] = [
  {
    label: "Documentation",
    icon: BookOpen,
    href: "https://ton-blockchain.github.io/acton/docs/welcome",
  },
  {label: "GitHub", icon: Github, href: "https://github.com/ton-blockchain/acton"},
]

export const DashboardNavigation: FC<DashboardNavigationProps> = ({
  client,
  localnetApiToken,
  onOpenAuthTokenOverlay,
  theme,
  setTheme,
  onToggleSidebar,
  isSidebarCollapsed = false,
  className,
}) => {
  const location = useLocation()
  const navigate = useNavigate()
  const {forkNetwork} = useNetworkInfo()
  const [explorerPath, setExplorerPath] = useState(() => readExplorerLastPath())
  const [mobileMenuOpen, setMobileMenuOpen] = useState(false)
  const forkBadgeLabel = useMemo(() => formatForkNetworkLabel(forkNetwork), [forkNetwork])
  const closeMobileMenu = useCallback(() => setMobileMenuOpen(false), [])

  useEffect(() => {
    if (
      !location.pathname.startsWith("/explorer") ||
      location.pathname === "/explorer/blocks" ||
      location.pathname === "/explorer/sources" ||
      location.pathname.startsWith("/explorer/abi") ||
      location.pathname === "/explorer/favorites"
    ) {
      return
    }

    const nextPath = `${location.pathname}${location.search}${location.hash}`
    writeExplorerLastPath(nextPath)
    setExplorerPath(nextPath)
  }, [location.hash, location.pathname, location.search])

  useEffect(() => {
    closeMobileMenu()
  }, [closeMobileMenu, location.hash, location.pathname, location.search])

  useEffect(() => {
    if (!mobileMenuOpen) {
      return
    }

    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        closeMobileMenu()
      }
    }

    globalThis.addEventListener("keydown", onKeyDown)
    return () => globalThis.removeEventListener("keydown", onKeyDown)
  }, [closeMobileMenu, mobileMenuOpen])

  const renderWorkspaceHeader = () => (
    <div className={styles.workspaceHeader}>
      <span className={styles.workspaceMark} />
      <span className={styles.workspaceBody}>
        <span className={styles.workspaceTitleRow}>
          <span className={styles.workspaceName}>TON Localnet</span>
          {forkBadgeLabel && <span className={styles.workspaceForkBadge}>{forkBadgeLabel}</span>}
        </span>
        <span className={styles.workspaceMeta}>by Acton</span>
      </span>
    </div>
  )

  return (
    <>
      <header className={styles.mobileTopbar}>
        {renderWorkspaceHeader()}
        <button
          type="button"
          className={styles.mobileMenuButton}
          aria-label="Open navigation menu"
          aria-expanded={mobileMenuOpen}
          onClick={() => setMobileMenuOpen(true)}
        >
          <Menu size={20} />
        </button>
      </header>

      <button
        type="button"
        className={`${styles.mobileBackdrop} ${mobileMenuOpen ? styles.mobileBackdropOpen : ""}`}
        aria-label="Close navigation menu"
        tabIndex={mobileMenuOpen ? 0 : -1}
        onClick={closeMobileMenu}
      />

      <aside
        className={`${styles.sidebar} ${mobileMenuOpen ? styles.sidebarOpen : ""} ${className ?? ""}`}
        aria-label="Main navigation"
      >
        <div className={styles.sidebarHeader}>
          {renderWorkspaceHeader()}
          <button
            type="button"
            className={styles.mobileCloseButton}
            aria-label="Close navigation menu"
            onClick={closeMobileMenu}
          >
            <X size={20} />
          </button>
        </div>

        <div className={styles.topControls}>
          <DashboardSearch client={client} />
        </div>

        <div className={styles.navScroll}>
          <nav className={styles.nav}>
            {navigationSections.map((section, index) => (
              <Fragment key={section.id}>
                {index > 0 && <div className={styles.navDivider} />}
                <div className={styles.navSection}>
                  {section.items.map(item => {
                    const Icon = item.icon
                    const targetPath = item.path === "/explorer" ? explorerPath : item.path
                    const isActive =
                      item.path === "/explorer"
                        ? location.pathname.startsWith("/explorer") &&
                          location.pathname !== "/explorer/blocks" &&
                          location.pathname !== "/explorer/sources" &&
                          !location.pathname.startsWith("/explorer/abi") &&
                          location.pathname !== "/explorer/favorites"
                        : item.path === "/explorer/blocks"
                          ? location.pathname === "/explorer/blocks" ||
                            location.pathname === "/blocks" ||
                            location.pathname.startsWith("/block/")
                          : item.path === "/explorer/abi"
                            ? location.pathname.startsWith("/explorer/abi")
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
                          closeMobileMenu()
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
              </Fragment>
            ))}

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
                      onClick={closeMobileMenu}
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

              <div className={styles.navUtilityRow}>
                {onToggleSidebar && (
                  <button
                    type="button"
                    className={styles.sidebarToggleButton}
                    onClick={onToggleSidebar}
                    title={isSidebarCollapsed ? "Pin navigation" : "Collapse navigation"}
                    aria-label={isSidebarCollapsed ? "Pin navigation" : "Collapse navigation"}
                  >
                    {isSidebarCollapsed ? (
                      <PanelLeftOpen size={18} />
                    ) : (
                      <PanelLeftClose size={18} />
                    )}
                  </button>
                )}

                <button
                  type="button"
                  className={`${styles.sidebarUtilityButton} ${
                    localnetApiToken ? styles.sidebarUtilityButtonActive : ""
                  }`}
                  onClick={() => {
                    onOpenAuthTokenOverlay()
                    closeMobileMenu()
                  }}
                  title={localnetApiToken ? "Localnet API token set" : "Set localnet API token"}
                  aria-label={
                    localnetApiToken ? "Edit localnet API token" : "Set localnet API token"
                  }
                >
                  <KeyRound size={18} />
                  {localnetApiToken ? (
                    <Check size={12} className={styles.utilityStatusIcon} />
                  ) : undefined}
                </button>

                <button
                  type="button"
                  className={`${styles.sidebarUtilityButton} ${
                    location.pathname === "/explorer/favorites"
                      ? styles.sidebarUtilityButtonActive
                      : ""
                  }`}
                  onClick={() => {
                    void navigate("/explorer/favorites")
                    closeMobileMenu()
                  }}
                  title="Favorite accounts"
                  aria-label="Favorite accounts"
                >
                  <Star size={18} />
                </button>

                <ThemeSwitch
                  theme={theme}
                  onToggleTheme={() => setTheme(theme === "light" ? "dark" : "light")}
                />
              </div>
            </div>
          </nav>
        </div>
      </aside>
    </>
  )
}

function formatForkNetworkLabel(forkNetwork?: string): string | undefined {
  const normalizedForkNetwork = forkNetwork?.trim()
  if (!normalizedForkNetwork) {
    return undefined
  }

  return `${normalizedForkNetwork.toLocaleLowerCase()} fork`
}
