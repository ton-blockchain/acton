import {
  ArrowUpRight,
  BookOpen,
  ChevronRight,
  Github,
  Globe2,
  Menu,
  PanelLeftClose,
  PanelLeftOpen,
  X,
} from "lucide-react"
import {Fragment, useCallback, useEffect, useState} from "react"
import type {ReactNode} from "react"
import {ThemeSwitch, Tooltip} from "@acton/ui"

import type {StudioEnvironment, TestRunSummary} from "../studioApi"
import type {StudioPage, StudioPath} from "../studioPages"
import {StudioSearch} from "./StudioSearch"
import {TestRunsNavigationList} from "./TestRunsNavigationList"

import styles from "./StudioNavigation.module.css"

const NESTED_NAVIGATION_LIMIT = 5

interface StudioNavigationProps {
  readonly activePath: StudioPath
  readonly activeEnvironmentId?: string
  readonly activeNetworkId?: string
  readonly className?: string
  readonly contextAction?: {
    readonly label: string
    readonly onSelect: () => void
  }
  readonly environments?: readonly StudioEnvironment[]
  readonly networks?: readonly StudioEnvironment[]
  readonly isSidebarCollapsed?: boolean
  readonly navigationContent?: ReactNode
  readonly navigationKey?: string
  readonly pages: readonly StudioPage[]
  readonly searchContent?: ReactNode
  readonly selectedTestRunId?: string
  readonly testRuns?: readonly TestRunSummary[]
  readonly utilityActions?: ReactNode
  readonly onNavigate: (path: StudioPath) => void
  readonly onOpenEnvironment?: (environment: StudioEnvironment) => void
  readonly onSelectTestRun?: (runId: string) => void
  readonly onToggleSidebar?: () => void
}

export function StudioNavigation({
  activePath,
  activeEnvironmentId,
  activeNetworkId,
  className,
  contextAction,
  environments = [],
  networks = [],
  isSidebarCollapsed = false,
  navigationContent,
  navigationKey = "studio",
  pages,
  searchContent,
  selectedTestRunId,
  testRuns = [],
  utilityActions,
  onNavigate,
  onOpenEnvironment,
  onSelectTestRun,
  onToggleSidebar,
}: StudioNavigationProps) {
  const [mobileMenuOpen, setMobileMenuOpen] = useState(false)
  const closeMobileMenu = useCallback(() => setMobileMenuOpen(false), [])
  const navigateAndClose = useCallback(
    (path: StudioPath) => {
      onNavigate(path)
      closeMobileMenu()
    },
    [closeMobileMenu, onNavigate],
  )
  const openEnvironmentAndClose = useCallback(
    (environment: StudioEnvironment) => {
      onOpenEnvironment?.(environment)
      closeMobileMenu()
    },
    [closeMobileMenu, onOpenEnvironment],
  )
  const selectTestRunAndClose = useCallback(
    (runId: string) => {
      onSelectTestRun?.(runId)
      closeMobileMenu()
    },
    [closeMobileMenu, onSelectTestRun],
  )
  const navigationEnvironments = environments.slice(0, NESTED_NAVIGATION_LIMIT)
  const navigationTestRuns = testRuns.slice(0, NESTED_NAVIGATION_LIMIT)

  useEffect(() => {
    if (!mobileMenuOpen) return

    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") closeMobileMenu()
    }

    globalThis.addEventListener("keydown", onKeyDown)
    return () => globalThis.removeEventListener("keydown", onKeyDown)
  }, [closeMobileMenu, mobileMenuOpen])

  const renderWorkspaceHeader = () => (
    <div className={styles.workspaceHeader}>
      <span className={styles.workspaceMark} />
      <span className={styles.workspaceBody}>
        <span className={styles.workspaceTitleRow}>
          <span className={styles.workspaceName}>Acton Studio</span>
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
          {searchContent ?? <StudioSearch onNavigate={navigateAndClose} />}
        </div>

        <div className={styles.navigationFrame}>
          <div className={styles.navScroll}>
            <div key={navigationKey} className={styles.navigationPanel}>
              {navigationContent ?? (
                <nav className={styles.nav}>
                  {contextAction ? (
                    <button
                      type="button"
                      className={styles.navigationContextButton}
                      onClick={contextAction.onSelect}
                    >
                      <span>{contextAction.label}</span>
                      <ChevronRight size={17} aria-hidden="true" />
                    </button>
                  ) : undefined}

                  <div className={styles.navBody}>
                    <div className={styles.navSection}>
                      {pages.map(page => {
                        const Icon = page.icon
                        const isActive =
                          page.path === activePath &&
                          !activeNetworkId &&
                          !(page.path === "/virtual-environments" && activeEnvironmentId)
                        const showEnvironments =
                          page.path === "/virtual-environments" &&
                          activePath === "/virtual-environments" &&
                          navigationEnvironments.length > 0
                        const showTestRuns =
                          page.path === "/tests" &&
                          activePath === "/tests" &&
                          navigationTestRuns.length > 0

                        return (
                          <Fragment key={page.path}>
                            <button
                              type="button"
                              className={`${styles.navItem} ${isActive ? styles.navItemActive : ""}`}
                              aria-current={isActive ? "page" : undefined}
                              onClick={() => navigateAndClose(page.path)}
                            >
                              <span className={styles.navItemMain}>
                                <Icon size={18} />
                                <span>{page.label}</span>
                              </span>
                            </button>
                            {page.path === "/virtual-environments" &&
                            navigationEnvironments.length > 0 ? (
                              <div
                                className={`${styles.environmentNavDisclosure} ${
                                  showEnvironments ? styles.environmentNavDisclosureOpen : ""
                                }`}
                                aria-hidden={!showEnvironments}
                              >
                                <div className={styles.environmentNavClip}>
                                  <ul
                                    className={styles.environmentNavList}
                                    aria-label="Virtual environments"
                                  >
                                    {navigationEnvironments.map(environment => (
                                      <li key={environment.id}>
                                        <button
                                          type="button"
                                          className={`${styles.environmentNavItem} ${
                                            activeEnvironmentId === environment.id
                                              ? styles.environmentNavItemActive
                                              : ""
                                          }`}
                                          aria-current={
                                            activeEnvironmentId === environment.id
                                              ? "page"
                                              : undefined
                                          }
                                          tabIndex={showEnvironments ? 0 : -1}
                                          onClick={() => openEnvironmentAndClose(environment)}
                                        >
                                          <span className={styles.environmentNavName}>
                                            {environment.name}
                                          </span>
                                          <span
                                            className={styles.environmentStatusDot}
                                            data-status={environment.status}
                                            role="img"
                                            aria-label={`Status: ${environment.status}`}
                                            title={environment.status}
                                          />
                                        </button>
                                      </li>
                                    ))}
                                  </ul>
                                </div>
                              </div>
                            ) : undefined}
                            {page.path === "/tests" && navigationTestRuns.length > 0 ? (
                              <div
                                className={`${styles.environmentNavDisclosure} ${
                                  showTestRuns ? styles.environmentNavDisclosureOpen : ""
                                }`}
                                aria-hidden={!showTestRuns}
                              >
                                <div className={styles.environmentNavClip}>
                                  <TestRunsNavigationList
                                    runs={navigationTestRuns}
                                    selectedRunId={selectedTestRunId}
                                    onSelect={selectTestRunAndClose}
                                  />
                                </div>
                              </div>
                            ) : undefined}
                          </Fragment>
                        )
                      })}
                      {networks.length > 0 ? (
                        <div className={styles.networkNavGroup}>
                          <div className={styles.networkNavLabel}>
                            <Globe2 size={18} aria-hidden="true" />
                            <span>Networks</span>
                          </div>
                          <ul className={styles.environmentNavList} aria-label="Networks">
                            {networks.map(network => (
                              <li key={network.id}>
                                <button
                                  type="button"
                                  className={`${styles.environmentNavItem} ${
                                    activeNetworkId === network.id
                                      ? styles.environmentNavItemActive
                                      : ""
                                  }`}
                                  aria-current={activeNetworkId === network.id ? "page" : undefined}
                                  onClick={() => openEnvironmentAndClose(network)}
                                >
                                  <span className={styles.networkNavIdentity}>
                                    <span className={styles.environmentNavName}>
                                      {network.name}
                                    </span>
                                    {!network.network.testOnly && (
                                      <span
                                        className={styles.networkNavBadge}
                                        title="Live production network"
                                      >
                                        Live
                                      </span>
                                    )}
                                  </span>
                                  <span
                                    className={styles.environmentStatusDot}
                                    data-status={network.status}
                                    role="img"
                                    aria-label={`Status: ${network.status}`}
                                    title={network.status}
                                  />
                                </button>
                              </li>
                            ))}
                          </ul>
                        </div>
                      ) : undefined}
                    </div>
                  </div>
                </nav>
              )}
            </div>
          </div>

          <div className={styles.navFooter}>
            <div className={styles.navSection}>
              <a
                className={styles.navItem}
                href="https://ton-blockchain.github.io/acton/docs/welcome"
                target="_blank"
                rel="noreferrer"
                onClick={closeMobileMenu}
              >
                <span className={styles.navItemMain}>
                  <BookOpen size={18} />
                  <span>Documentation</span>
                </span>
                <ArrowUpRight size={14} />
              </a>
              <a
                className={styles.navItem}
                href="https://github.com/ton-blockchain/acton"
                target="_blank"
                rel="noreferrer"
                onClick={closeMobileMenu}
              >
                <span className={styles.navItemMain}>
                  <Github size={18} />
                  <span>GitHub</span>
                </span>
                <ArrowUpRight size={14} />
              </a>
            </div>

            <div className={styles.navUtilityRow}>
              <div className={styles.navUtilityStart}>
                {onToggleSidebar && (
                  <Tooltip content={isSidebarCollapsed ? "Pin navigation" : "Collapse navigation"}>
                    <button
                      type="button"
                      className={styles.sidebarToggleButton}
                      aria-label={isSidebarCollapsed ? "Pin navigation" : "Collapse navigation"}
                      onClick={onToggleSidebar}
                    >
                      {isSidebarCollapsed ? (
                        <PanelLeftOpen size={18} />
                      ) : (
                        <PanelLeftClose size={18} />
                      )}
                    </button>
                  </Tooltip>
                )}
                {utilityActions}
              </div>
              <ThemeSwitch />
            </div>
          </div>
        </div>
      </aside>
    </>
  )
}
