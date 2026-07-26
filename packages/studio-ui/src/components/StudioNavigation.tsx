import {
  ArrowUpRight,
  BookOpen,
  ChevronRight,
  Github,
  Menu,
  PanelLeftClose,
  PanelLeftOpen,
  X,
} from "lucide-react"
import {Fragment, useCallback, useEffect, useState} from "react"
import type {ReactNode} from "react"
import {ThemeSwitch, Tooltip} from "@acton/ui"

import type {StudioEnvironment} from "../studioApi"
import type {StudioPage, StudioPath} from "../studioPages"
import {StudioSearch} from "./StudioSearch"

import styles from "./StudioNavigation.module.css"

const ENVIRONMENT_NAVIGATION_LIMIT = 5

interface StudioNavigationProps {
  readonly activePath: StudioPath
  readonly activeEnvironmentId?: string
  readonly className?: string
  readonly contextAction?: {
    readonly label: string
    readonly onSelect: () => void
  }
  readonly environments?: readonly StudioEnvironment[]
  readonly isSidebarCollapsed?: boolean
  readonly navigationContent?: ReactNode
  readonly navigationKey?: string
  readonly pages: readonly StudioPage[]
  readonly searchContent?: ReactNode
  readonly utilityActions?: ReactNode
  readonly onNavigate: (path: StudioPath) => void
  readonly onOpenEnvironment?: (environment: StudioEnvironment) => void
  readonly onToggleSidebar?: () => void
}

export function StudioNavigation({
  activePath,
  activeEnvironmentId,
  className,
  contextAction,
  environments = [],
  isSidebarCollapsed = false,
  navigationContent,
  navigationKey = "studio",
  pages,
  searchContent,
  utilityActions,
  onNavigate,
  onOpenEnvironment,
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
  const navigationEnvironments = environments.slice(0, ENVIRONMENT_NAVIGATION_LIMIT)

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
                          !(page.path === "/virtual-environments" && activeEnvironmentId)
                        const showEnvironments =
                          page.path === "/virtual-environments" &&
                          activePath === "/virtual-environments" &&
                          navigationEnvironments.length > 0

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
                          </Fragment>
                        )
                      })}
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
