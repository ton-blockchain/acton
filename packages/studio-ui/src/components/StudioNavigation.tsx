import {ArrowUpRight, BookOpen, Github, Menu, PanelLeftClose, PanelLeftOpen, X} from "lucide-react"
import {useCallback, useEffect, useState} from "react"
import {ThemeSwitch, Tooltip} from "@acton/ui"

import type {StudioPage, StudioPath} from "../studioPages"
import {StudioSearch} from "./StudioSearch"

import styles from "./StudioNavigation.module.css"

interface StudioNavigationProps {
  readonly activePath: StudioPath
  readonly className?: string
  readonly isSidebarCollapsed?: boolean
  readonly pages: readonly StudioPage[]
  readonly onNavigate: (path: StudioPath) => void
  readonly onToggleSidebar?: () => void
}

export function StudioNavigation({
  activePath,
  className,
  isSidebarCollapsed = false,
  pages,
  onNavigate,
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
          <StudioSearch onNavigate={navigateAndClose} />
        </div>

        <div className={styles.navScroll}>
          <nav className={styles.nav}>
            <div className={styles.navSection}>
              {pages.map(page => {
                const Icon = page.icon
                const isActive = page.path === activePath

                return (
                  <button
                    key={page.path}
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
                )
              })}
            </div>

            <div className={styles.navDivider} />

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
                <ThemeSwitch />
              </div>
            </div>
          </nav>
        </div>
      </aside>
    </>
  )
}
