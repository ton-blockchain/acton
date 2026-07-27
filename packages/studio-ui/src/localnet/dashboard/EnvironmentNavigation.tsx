import {useEffect, useMemo, useState} from "react"
import type {FC} from "react"
import {
  Activity,
  Binary,
  Brackets,
  ChevronLeft,
  ChevronRight,
  FileJson,
  HandCoins,
  LayoutGrid,
  Search as SearchIcon,
  Settings2,
  Wallet,
  Waypoints,
} from "lucide-react"
import type {LucideIcon} from "lucide-react"
import {useLocation, useNavigate} from "react-router"

import {readExplorerLastPath, writeExplorerLastPath} from "../explorer/explorerResume"
import {useNetworkInfo} from "../explorer/hooks/useNetworkInfo"
import {useLocalnetRoutes} from "../routes"
import {formatForkNetworkLabel} from "./dashboardUtils"

import styles from "./DashboardPage.module.css"

interface EnvironmentNavigationProps {
  readonly environmentName: string
  readonly onShowStudioNavigation: () => void
}

interface SidebarItem {
  readonly label: string
  readonly icon: LucideIcon
  readonly path: string
}

interface NestedSidebarItem {
  readonly label: string
  readonly path: string
}

const primaryItems: SidebarItem[] = [
  {label: "Home", icon: LayoutGrid, path: "/dashboard"},
]

const explorerItems: NestedSidebarItem[] = [
  {label: "Overview", path: "/explorer"},
  {label: "Blocks", path: "/explorer/blocks"},
  {label: "Sources", path: "/explorer/sources"},
  {label: "ABI", path: "/explorer/abi"},
  {label: "Tokens", path: "/explorer/tokens"},
  {label: "NFTs", path: "/explorer/nfts"},
]

const standaloneItems: SidebarItem[] = [
  {label: "Simulator", icon: Waypoints, path: "/simulator"},
  {label: "Cell Inspector", icon: Binary, path: "/cell-inspector"},
]

const environmentItems: SidebarItem[] = [
  {label: "Wallets", icon: Wallet, path: "/wallets"},
  {label: "Faucet", icon: HandCoins, path: "/faucet"},
]

const apiItems: SidebarItem[] = [
  {label: "API Calls", icon: Activity, path: "/api-calls"},
  {label: "v2 API", icon: FileJson, path: "/api-reference/v2"},
  {label: "v3 API", icon: Brackets, path: "/api-reference/v3"},
  {label: "Control API", icon: Settings2, path: "/api-reference/control"},
]

const navigationSections: Array<{readonly id: string; readonly items: readonly SidebarItem[]}> = [
  {id: "environment", items: environmentItems},
  {id: "api", items: apiItems},
]

export const EnvironmentNavigation: FC<EnvironmentNavigationProps> = ({
  environmentName,
  onShowStudioNavigation,
}) => {
  const location = useLocation()
  const navigate = useNavigate()
  const routes = useLocalnetRoutes()
  const {forkNetwork} = useNetworkInfo()
  const [explorerPath, setExplorerPath] = useState(() => readExplorerLastPath())
  const forkBadgeLabel = useMemo(() => formatForkNetworkLabel(forkNetwork), [forkNetwork])
  const localPathname = location.pathname.slice(routes.basePath.length) || "/"
  const isExplorerActive =
    localPathname.startsWith("/explorer") || localPathname.startsWith("/block/")
  const isExplorerOverviewActive =
    localPathname.startsWith("/explorer") &&
    localPathname !== "/explorer/blocks" &&
    localPathname !== "/explorer/sources" &&
    !localPathname.startsWith("/explorer/abi") &&
    localPathname !== "/explorer/tokens" &&
    localPathname !== "/explorer/nfts" &&
    localPathname !== "/explorer/favorites"
  const [isExplorerOpen, setIsExplorerOpen] = useState(isExplorerActive)

  useEffect(() => {
    if (isExplorerActive) setIsExplorerOpen(true)
  }, [isExplorerActive])

  useEffect(() => {
    if (
      !localPathname.startsWith("/explorer") ||
      localPathname === "/explorer/blocks" ||
      localPathname === "/explorer/sources" ||
      localPathname.startsWith("/explorer/abi") ||
      localPathname === "/explorer/tokens" ||
      localPathname === "/explorer/nfts" ||
      localPathname === "/explorer/favorites"
    ) {
      return
    }

    const nextPath = `${localPathname}${location.search}${location.hash}`
    writeExplorerLastPath(nextPath)
    setExplorerPath(nextPath)
  }, [localPathname, location.hash, location.search])

  return (
    <nav className={styles.environmentNavigation} aria-label={`${environmentName} navigation`}>
      <button
        type="button"
        className={styles.environmentContext}
        aria-label="Show Studio navigation"
        title={environmentName}
        onClick={onShowStudioNavigation}
      >
        <ChevronLeft size={19} aria-hidden="true" />
        <span className={styles.environmentContextTitle}>
          <span className={styles.environmentContextName}>{environmentName}</span>
          {forkBadgeLabel ? (
            <span className={styles.workspaceForkBadge}>{forkBadgeLabel}</span>
          ) : undefined}
        </span>
        <span aria-hidden="true" />
      </button>

      <div className={styles.environmentNavBody}>
        <div className={styles.navSection}>
          {primaryItems.map(item => {
            const Icon = item.icon
            const isActive = item.path === localPathname

            return (
              <button
                type="button"
                key={item.label}
                className={`${styles.navItem} ${isActive ? styles.navItemActive : ""}`}
                onClick={() => void navigate(routes.path(item.path))}
              >
                <span className={styles.navItemMain}>
                  <Icon size={18} />
                  <span>{item.label}</span>
                </span>
              </button>
            )
          })}

          <div className={styles.explorerNavGroup}>
            <div
              className={`${styles.explorerNavRow} ${
                isExplorerActive ? styles.explorerNavRowActive : ""
              }`}
            >
              <button
                type="button"
                className={styles.navItem}
                onClick={() => void navigate(routes.path(explorerPath))}
              >
                <span className={styles.navItemMain}>
                  <SearchIcon size={18} />
                  <span>Explorer</span>
                </span>
              </button>
              <button
                type="button"
                className={styles.explorerNavToggle}
                aria-controls="environment-explorer-navigation"
                aria-expanded={isExplorerOpen}
                aria-label={`${isExplorerOpen ? "Collapse" : "Expand"} Explorer pages`}
                onClick={() => setIsExplorerOpen(open => !open)}
              >
                <ChevronRight
                  className={isExplorerOpen ? styles.explorerNavChevronOpen : undefined}
                  size={17}
                  aria-hidden="true"
                />
              </button>
            </div>

            <div
              id="environment-explorer-navigation"
              className={`${styles.explorerNavDisclosure} ${
                isExplorerOpen ? styles.explorerNavDisclosureOpen : ""
              }`}
              aria-hidden={!isExplorerOpen}
            >
              <div className={styles.explorerNavClip}>
                <ul className={styles.explorerNavList} aria-label="Explorer pages">
                  {explorerItems.map(item => {
                    const isActive =
                      item.path === "/explorer"
                        ? isExplorerOverviewActive
                        : item.path === "/explorer/blocks"
                        ? localPathname === item.path || localPathname.startsWith("/block/")
                        : item.path === "/explorer/abi"
                          ? localPathname.startsWith(item.path)
                          : localPathname === item.path

                    return (
                      <li key={item.label}>
                        <button
                          type="button"
                          className={`${styles.explorerNavItem} ${
                            isActive ? styles.explorerNavItemActive : ""
                          }`}
                          aria-current={isActive ? "page" : undefined}
                          tabIndex={isExplorerOpen ? 0 : -1}
                          onClick={() => void navigate(routes.path(item.path))}
                        >
                          {item.label}
                        </button>
                      </li>
                    )
                  })}
                </ul>
              </div>
            </div>
          </div>

          {standaloneItems.map(item => {
            const Icon = item.icon
            const isActive = item.path === localPathname

            return (
              <button
                type="button"
                key={item.label}
                className={`${styles.navItem} ${isActive ? styles.navItemActive : ""}`}
                onClick={() => void navigate(routes.path(item.path))}
              >
                <span className={styles.navItemMain}>
                  <Icon size={18} />
                  <span>{item.label}</span>
                </span>
              </button>
            )
          })}
        </div>

        {navigationSections.map(section => (
          <div className={styles.navigationSectionGroup} key={section.id}>
            <div className={styles.navDivider} />
            <div className={styles.navSection}>
              {section.items.map(item => {
                const Icon = item.icon
                const isActive = item.path === localPathname

                return (
                  <button
                    type="button"
                    key={item.label}
                    className={`${styles.navItem} ${isActive ? styles.navItemActive : ""}`}
                    onClick={() => void navigate(routes.path(item.path))}
                  >
                    <span className={styles.navItemMain}>
                      <Icon size={18} />
                      <span>{item.label}</span>
                    </span>
                  </button>
                )
              })}
            </div>
          </div>
        ))}
      </div>
    </nav>
  )
}
