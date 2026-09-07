import {useEffect, useRef, useState} from "react"
import type {FC} from "react"
import {
  Box,
  Brackets,
  ChevronLeft,
  ChevronRight,
  RadioTower,
  Search as SearchIcon,
} from "lucide-react"
import type {LucideIcon} from "lucide-react"
import {useLocation, useNavigate} from "react-router"

import {supports} from "../../environmentCapabilities"
import {
  getEnvironmentNavigation,
  type SidebarItem,
  type NestedSidebarItem,
} from "./environmentNavigationItems"
import {useLocalnetRuntime} from "../LocalnetRuntimeProvider"
import {useNetworkInfo} from "@acton/explorer-core/hooks/useNetworkInfo"
import {useLocalnetRoutes} from "../routes"
import {formatForkNetworkLabel} from "./dashboardUtils"

import styles from "./DashboardPage.module.css"

interface EnvironmentNavigationProps {
  readonly environmentName: string
  readonly onShowStudioNavigation: () => void
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
  const {forkNetwork} = useNetworkInfo()
  const navigationRef = useRef<HTMLElement>(null)
  const forkBadgeLabel =
    environment?.config.kind === "actonSimulatedLocalnet"
      ? formatForkNetworkLabel(forkNetwork)
      : undefined
  const {
    primaryItems,
    visibleStandaloneItems,
    visibleExplorerItems,
    visibleNetworkItems,
    visibleEnvironmentItems,
    visibleApiReferenceItems,
    contractItems,
    integrateItem,
    apiCallsItem,
  } = getEnvironmentNavigation(environment)
  const localPathname = location.pathname.slice(routes.basePath.length) || "/"
  const isExplorerActive =
    localPathname.startsWith("/explorer") || localPathname.startsWith("/block/")
  const isExplorerOverviewActive =
    localPathname.startsWith("/explorer") &&
    localPathname !== "/explorer/blocks" &&
    !localPathname.startsWith("/explorer/config") &&
    localPathname !== "/explorer/elections" &&
    localPathname !== "/explorer/tokens" &&
    localPathname !== "/explorer/nfts" &&
    localPathname !== "/explorer/favorites"
  const [isExplorerOpen, setIsExplorerOpen] = useState(isExplorerActive)
  const isNetworkActive = localPathname.startsWith("/network")
  const [isNetworkOpen, setIsNetworkOpen] = useState(isNetworkActive)
  const isContractsActive = localPathname.startsWith("/contracts")
  const [isContractsOpen, setIsContractsOpen] = useState(isContractsActive)
  const isApiReferenceActive = localPathname.startsWith("/api-reference/")
  const [isApiReferenceOpen, setIsApiReferenceOpen] = useState(isApiReferenceActive)

  useEffect(() => {
    if (isExplorerActive) setIsExplorerOpen(true)
  }, [isExplorerActive])

  useEffect(() => {
    if (isNetworkActive) setIsNetworkOpen(true)
  }, [isNetworkActive])

  useEffect(() => {
    if (isContractsActive) setIsContractsOpen(true)
  }, [isContractsActive])

  useEffect(() => {
    if (isApiReferenceActive) setIsApiReferenceOpen(true)
  }, [isApiReferenceActive])

  useEffect(() => {
    // Wait until a newly opened group reaches its final height. Revealing the
    // active child while the grid row is still growing makes the scroll
    // container correct its position again at the end of the transition.
    const timeout = globalThis.setTimeout(() => {
      navigationRef.current
        ?.querySelector<HTMLElement>('[aria-current="page"]')
        ?.scrollIntoView({block: "nearest", inline: "nearest"})
    }, 280)

    return () => globalThis.clearTimeout(timeout)
  }, [localPathname])

  return (
    <nav
      ref={navigationRef}
      className={styles.environmentNavigation}
      aria-label={`${environmentName} navigation`}
    >
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

          {visibleNetworkItems.length > 0 ? (
            <NavigationDisclosure
              active={isNetworkActive}
              ariaLabel="Network pages"
              controlsId="environment-network-navigation"
              icon={RadioTower}
              isItemActive={item => localPathname === item.path}
              items={visibleNetworkItems}
              label="Network"
              onItemSelect={path => void navigate(routes.path(path))}
              onParentSelect={() => {
                // Open in the same render as the route change so the sidebar
                // never paints the active group in its collapsed state.
                setIsNetworkOpen(true)
                void navigate(
                  routes.path(
                    environment?.config.kind === "fullTonNetwork" ? "/network" : "/network/config",
                  ),
                )
              }}
              onToggle={() => setIsNetworkOpen(open => !open)}
              open={isNetworkOpen}
            />
          ) : undefined}

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
                    : item.path === "/explorer/config"
                      ? localPathname.startsWith(item.path)
                      : localPathname === item.path
              }
              items={visibleExplorerItems}
              label="Explorer"
              onItemSelect={path => void navigate(routes.path(path))}
              onParentSelect={() => {
                setIsExplorerOpen(true)
                void navigate(routes.path("/explorer"))
              }}
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
              onParentSelect={() => {
                setIsContractsOpen(true)
                void navigate(routes.path("/contracts"))
              }}
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
              {integrateItem ? (
                <NavigationItem
                  active={integrateItem.path === localPathname}
                  item={integrateItem}
                  onSelect={path => void navigate(routes.path(path))}
                />
              ) : undefined}
              {apiCallsItem ? (
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
                  onParentSelect={() => {
                    setIsApiReferenceOpen(true)
                    void navigate(routes.path(visibleApiReferenceItems[0].path))
                  }}
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
