import {useEffect, useMemo, useState} from "react"
import type {FC} from "react"
import {
  Activity,
  Binary,
  Box,
  Brackets,
  Cable,
  ChevronLeft,
  ChevronRight,
  HandCoins,
  LayoutGrid,
  Search as SearchIcon,
  Wallet,
  Waypoints,
} from "lucide-react"
import type {LucideIcon} from "lucide-react"
import {useLocation, useNavigate} from "react-router"

import {supports, supportsAny} from "../../environmentCapabilities"
import {useLocalnetRuntime} from "../LocalnetRuntimeProvider"
import {readExplorerLastPath, writeExplorerLastPath} from "@acton/explorer-core/explorerResume"
import {useNetworkInfo} from "@acton/explorer-core/hooks/useNetworkInfo"
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

interface NavigationItemProps {
  readonly active: boolean
  readonly item: SidebarItem
  readonly onSelect: (path: string) => void
}

interface NavigationDisclosureProps {
  readonly active: boolean
  readonly ariaLabel: string
  readonly controlsId: string
  readonly icon: LucideIcon
  readonly isItemActive: (item: NestedSidebarItem) => boolean
  readonly items: readonly NestedSidebarItem[]
  readonly label: string
  readonly onItemSelect: (path: string) => void
  readonly onParentSelect: () => void
  readonly onToggle: () => void
  readonly open: boolean
}

const primaryItems: SidebarItem[] = [{label: "Home", icon: LayoutGrid, path: "/dashboard"}]

const explorerItems: NestedSidebarItem[] = [
  {label: "Overview", path: "/explorer"},
  {label: "Blocks", path: "/explorer/blocks"},
  {label: "Tokens", path: "/explorer/tokens"},
  {label: "NFTs", path: "/explorer/nfts"},
]

const contractItems: NestedSidebarItem[] = [
  {label: "Overview", path: "/contracts"},
  {label: "Sources", path: "/contracts/sources"},
  {label: "ABI", path: "/contracts/abi"},
]

const standaloneItems: SidebarItem[] = [
  {label: "Simulator", icon: Waypoints, path: "/simulator"},
  {label: "Cell Inspector", icon: Binary, path: "/cell-inspector"},
]

const environmentItems: SidebarItem[] = [
  {label: "Wallets", icon: Wallet, path: "/wallets"},
  {label: "Faucet", icon: HandCoins, path: "/faucet"},
]

const apiCallsItem: SidebarItem = {
  label: "API Calls",
  icon: Activity,
  path: "/api-calls",
}

const integrateItem: SidebarItem = {
  label: "Integrate",
  icon: Cable,
  path: "/integrate",
}

const apiReferenceItems: NestedSidebarItem[] = [
  {label: "v2 API", path: "/api-reference/v2"},
  {label: "v3 API", path: "/api-reference/v3"},
  {label: "Control API", path: "/api-reference/control"},
]

const NavigationItem: FC<NavigationItemProps> = ({active, item, onSelect}) => {
  const Icon = item.icon

  return (
    <button
      type="button"
      className={`${styles.navItem} ${active ? styles.navItemActive : ""}`}
      aria-current={active ? "page" : undefined}
      onClick={() => onSelect(item.path)}
    >
      <span className={styles.navItemMain}>
        <Icon size={18} aria-hidden="true" />
        <span>{item.label}</span>
      </span>
    </button>
  )
}

const NavigationDisclosure: FC<NavigationDisclosureProps> = ({
  active,
  ariaLabel,
  controlsId,
  icon: Icon,
  isItemActive,
  items,
  label,
  onItemSelect,
  onParentSelect,
  onToggle,
  open,
}) => (
  <div className={styles.nestedNavGroup}>
    <div className={`${styles.nestedNavRow} ${active ? styles.nestedNavRowActive : ""}`}>
      <button type="button" className={styles.navItem} onClick={onParentSelect}>
        <span className={styles.navItemMain}>
          <Icon size={18} aria-hidden="true" />
          <span>{label}</span>
        </span>
      </button>
      <button
        type="button"
        className={styles.nestedNavToggle}
        aria-controls={controlsId}
        aria-expanded={open}
        aria-label={`${open ? "Collapse" : "Expand"} ${ariaLabel}`}
        onClick={onToggle}
      >
        <ChevronRight
          className={open ? styles.nestedNavChevronOpen : undefined}
          size={17}
          aria-hidden="true"
        />
      </button>
    </div>

    <div
      id={controlsId}
      className={`${styles.nestedNavDisclosure} ${open ? styles.nestedNavDisclosureOpen : ""}`}
      aria-hidden={!open}
    >
      <div className={styles.nestedNavClip}>
        <ul className={styles.nestedNavList} aria-label={ariaLabel}>
          {items.map(item => {
            const isActive = isItemActive(item)

            return (
              <li key={item.label}>
                <button
                  type="button"
                  className={`${styles.nestedNavItem} ${
                    isActive ? styles.nestedNavItemActive : ""
                  }`}
                  aria-current={isActive ? "page" : undefined}
                  tabIndex={open ? 0 : -1}
                  onClick={() => onItemSelect(item.path)}
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
)

export const EnvironmentNavigation: FC<EnvironmentNavigationProps> = ({
  environmentName,
  onShowStudioNavigation,
}) => {
  const location = useLocation()
  const navigate = useNavigate()
  const routes = useLocalnetRoutes()
  const {environment} = useLocalnetRuntime()
  const {forkNetwork, network} = useNetworkInfo()
  const [explorerPath, setExplorerPath] = useState(() => readExplorerLastPath())
  const forkBadgeLabel = useMemo(
    () =>
      formatForkNetworkLabel(forkNetwork) ??
      (network.id === "localnet" ? undefined : network.label),
    [forkNetwork, network.id, network.label],
  )
  const visibleStandaloneItems = supports(environment, "simulator") ? standaloneItems : []
  const visibleEnvironmentItems = environmentItems.filter(item =>
    item.path === "/wallets"
      ? supports(environment, "wallets")
      : supportsAny(environment, "gramFaucet", "jettonFaucet"),
  )
  const visibleApiReferenceItems = apiReferenceItems.filter(item =>
    item.path === "/api-reference/v2"
      ? supports(environment, "apiV2")
      : item.path === "/api-reference/v3"
        ? supports(environment, "apiV3")
        : supports(environment, "controlApi"),
  )
  const localPathname = location.pathname.slice(routes.basePath.length) || "/"
  const isExplorerActive =
    localPathname.startsWith("/explorer") || localPathname.startsWith("/block/")
  const isExplorerOverviewActive =
    localPathname.startsWith("/explorer") &&
    localPathname !== "/explorer/blocks" &&
    localPathname !== "/explorer/tokens" &&
    localPathname !== "/explorer/nfts" &&
    localPathname !== "/explorer/favorites"
  const [isExplorerOpen, setIsExplorerOpen] = useState(isExplorerActive)
  const isContractsActive = localPathname.startsWith("/contracts")
  const [isContractsOpen, setIsContractsOpen] = useState(isContractsActive)
  const isApiReferenceActive = localPathname.startsWith("/api-reference/")
  const [isApiReferenceOpen, setIsApiReferenceOpen] = useState(isApiReferenceActive)

  useEffect(() => {
    if (isExplorerActive) setIsExplorerOpen(true)
  }, [isExplorerActive])

  useEffect(() => {
    if (isContractsActive) setIsContractsOpen(true)
  }, [isContractsActive])

  useEffect(() => {
    if (isApiReferenceActive) setIsApiReferenceOpen(true)
  }, [isApiReferenceActive])

  useEffect(() => {
    if (
      !localPathname.startsWith("/explorer") ||
      localPathname === "/explorer/blocks" ||
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
          {primaryItems.map(item => (
            <NavigationItem
              key={item.label}
              active={item.path === localPathname}
              item={item}
              onSelect={path => void navigate(routes.path(path))}
            />
          ))}

          {supports(environment, "explorer") ? (
            <NavigationDisclosure
              active={isExplorerActive}
              ariaLabel="Explorer pages"
              controlsId="environment-explorer-navigation"
              icon={SearchIcon}
              isItemActive={item =>
                item.path === "/explorer"
                  ? isExplorerOverviewActive
                  : item.path === "/explorer/blocks"
                    ? localPathname === item.path || localPathname.startsWith("/block/")
                    : localPathname === item.path
              }
              items={explorerItems}
              label="Explorer"
              onItemSelect={path => void navigate(routes.path(path))}
              onParentSelect={() => void navigate(routes.path(explorerPath))}
              onToggle={() => setIsExplorerOpen(open => !open)}
              open={isExplorerOpen}
            />
          ) : undefined}

          {supports(environment, "contracts") ? (
            <NavigationDisclosure
              active={isContractsActive}
              ariaLabel="Contract pages"
              controlsId="environment-contract-navigation"
              icon={Box}
              isItemActive={item =>
                item.path === "/contracts"
                  ? localPathname === item.path
                  : item.path === "/contracts/abi"
                    ? localPathname.startsWith(item.path)
                    : localPathname === item.path
              }
              items={contractItems}
              label="Contracts"
              onItemSelect={path => void navigate(routes.path(path))}
              onParentSelect={() => void navigate(routes.path("/contracts"))}
              onToggle={() => setIsContractsOpen(open => !open)}
              open={isContractsOpen}
            />
          ) : undefined}

          {visibleStandaloneItems.map(item => (
            <NavigationItem
              key={item.label}
              active={item.path === localPathname}
              item={item}
              onSelect={path => void navigate(routes.path(path))}
            />
          ))}
        </div>

        {visibleEnvironmentItems.length > 0 ? (
          <div className={styles.navigationSectionGroup}>
            <div className={styles.navDivider} />
            <div className={styles.navSection}>
              {visibleEnvironmentItems.map(item => (
                <NavigationItem
                  key={item.label}
                  active={item.path === localPathname}
                  item={item}
                  onSelect={path => void navigate(routes.path(path))}
                />
              ))}
            </div>
          </div>
        ) : undefined}

        {supports(environment, "integration") ||
        supports(environment, "apiCalls") ||
        visibleApiReferenceItems.length > 0 ? (
          <div className={styles.navigationSectionGroup}>
            <div className={styles.navDivider} />
            <div className={styles.navSection}>
              {supports(environment, "integration") ? (
                <NavigationItem
                  active={integrateItem.path === localPathname}
                  item={integrateItem}
                  onSelect={path => void navigate(routes.path(path))}
                />
              ) : undefined}
              {supports(environment, "apiCalls") ? (
                <NavigationItem
                  active={apiCallsItem.path === localPathname}
                  item={apiCallsItem}
                  onSelect={path => void navigate(routes.path(path))}
                />
              ) : undefined}
              {visibleApiReferenceItems.length > 0 ? (
                <NavigationDisclosure
                  active={isApiReferenceActive}
                  ariaLabel="API Reference pages"
                  controlsId="environment-api-reference-navigation"
                  icon={Brackets}
                  isItemActive={item => localPathname === item.path}
                  items={visibleApiReferenceItems}
                  label="API Reference"
                  onItemSelect={path => void navigate(routes.path(path))}
                  onParentSelect={() =>
                    void navigate(routes.path(visibleApiReferenceItems[0].path))
                  }
                  onToggle={() => setIsApiReferenceOpen(open => !open)}
                  open={isApiReferenceOpen}
                />
              ) : undefined}
            </div>
          </div>
        ) : undefined}
      </div>
    </nav>
  )
}
