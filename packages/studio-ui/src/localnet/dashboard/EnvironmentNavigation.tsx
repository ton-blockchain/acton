import {Fragment, useEffect, useMemo, useState} from "react"
import type {FC} from "react"
import {
  Activity,
  Binary,
  Boxes,
  Brackets,
  ChevronLeft,
  Coins,
  FileCode2,
  FileJson,
  HandCoins,
  Image,
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
  readonly path?: string
  readonly href?: string
}

const mainItems: SidebarItem[] = [
  {label: "Home", icon: LayoutGrid, path: "/dashboard"},
  {label: "Explorer", icon: SearchIcon, path: "/explorer"},
  {label: "Simulator", icon: Waypoints, path: "/explorer/emulate"},
  {label: "Blocks", icon: Boxes, path: "/explorer/blocks"},
  {label: "Wallets", icon: Wallet, path: "/wallets"},
  {label: "Faucet", icon: HandCoins, path: "/faucet"},
  {label: "Tokens", icon: Coins, path: "/tokens"},
  {label: "NFTs", icon: Image, path: "/nfts"},
]

const sourceItems: SidebarItem[] = [
  {label: "Sources", icon: FileCode2, path: "/explorer/sources"},
  {label: "ABI", icon: FileJson, path: "/explorer/abi"},
  {label: "Cell Inspector", icon: Binary, path: "/explorer/cell"},
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

  useEffect(() => {
    if (
      !localPathname.startsWith("/explorer") ||
      localPathname === "/explorer/blocks" ||
      localPathname === "/explorer/sources" ||
      localPathname.startsWith("/explorer/abi") ||
      localPathname === "/explorer/cell" ||
      localPathname === "/explorer/emulate" ||
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
        {navigationSections.map((section, index) => (
          <Fragment key={section.id}>
            {index > 0 && <div className={styles.navDivider} />}
            <div className={styles.navSection}>
              {section.items.map(item => {
                const Icon = item.icon
                const targetPath = item.path === "/explorer" ? explorerPath : item.path
                const isActive =
                  item.path === "/explorer"
                    ? localPathname.startsWith("/explorer") &&
                      localPathname !== "/explorer/blocks" &&
                      localPathname !== "/explorer/sources" &&
                      !localPathname.startsWith("/explorer/abi") &&
                      localPathname !== "/explorer/cell" &&
                      localPathname !== "/explorer/emulate" &&
                      localPathname !== "/explorer/favorites"
                    : item.path === "/explorer/blocks"
                      ? localPathname === "/explorer/blocks" ||
                        localPathname === "/blocks" ||
                        localPathname.startsWith("/block/")
                      : item.path === "/explorer/abi"
                        ? localPathname.startsWith("/explorer/abi")
                        : item.path === localPathname

                return (
                  <button
                    type="button"
                    key={item.label}
                    className={`${styles.navItem} ${isActive ? styles.navItemActive : ""}`}
                    onClick={() => {
                      if (targetPath) void navigate(routes.path(targetPath))
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
      </div>
    </nav>
  )
}
