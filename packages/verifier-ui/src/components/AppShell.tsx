import {ThemeProvider, ThemeSwitch} from "@acton/ui"
import {useEffect, useRef, useState} from "react"
import type {ReactNode} from "react"
import {Github, Menu, Search, X} from "lucide-react"

import tonVerifierIcon from "../assets/ton-verifier-icons/icon.svg"
import {SearchBox} from "./SearchBox"
import styles from "./AppShell.module.css"

const PRIMARY_NAV_ITEMS = [
  {href: "/verified", label: "Verified contracts"},
  {href: "/statistics", label: "Statistics"},
] as const

interface AppShellProps {
  readonly children: ReactNode
  readonly headerAccessory?: ReactNode
}

export function AppShell({children, headerAccessory}: AppShellProps) {
  const {pathname} = globalThis.location
  const isHomePage = pathname === "/"
  const headerClassName = isHomePage ? `${styles.header} ${styles.headerHome}` : styles.header
  const [mobileHeaderPanel, setMobileHeaderPanel] = useState<"navigation" | "search">()
  const mobileNavigationRef = useRef<HTMLDivElement>(null)
  const mobileSearchRef = useRef<HTMLDivElement>(null)
  const mobileNavigationOpen = mobileHeaderPanel === "navigation"
  const mobileSearchOpen = mobileHeaderPanel === "search"

  useEffect(() => {
    if (!mobileHeaderPanel) return

    const closeMobileHeaderPanels = (event: PointerEvent) => {
      if (
        event.target instanceof Node &&
        (mobileNavigationRef.current?.contains(event.target) ||
          mobileSearchRef.current?.contains(event.target))
      ) {
        return
      }
      setMobileHeaderPanel(undefined)
    }
    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key === "Escape") setMobileHeaderPanel(undefined)
    }

    document.addEventListener("pointerdown", closeMobileHeaderPanels, true)
    document.addEventListener("keydown", closeOnEscape)
    return () => {
      document.removeEventListener("pointerdown", closeMobileHeaderPanels, true)
      document.removeEventListener("keydown", closeOnEscape)
    }
  }, [mobileHeaderPanel])

  return (
    <ThemeProvider storageKey="ton-verifier-theme">
      <div className={styles.appShell}>
        <header className={headerClassName}>
          <div className={styles.headerInner}>
            <div className={styles.headerPrimary}>
              <a className={styles.brand} href="/" aria-label="TON Verifier home">
                <img className={styles.brandIcon} src={tonVerifierIcon} alt="" aria-hidden="true" />
                <span>TON Verifier</span>
              </a>
              <nav className={styles.nav} aria-label="TON Verifier navigation">
                {PRIMARY_NAV_ITEMS.map(item => (
                  <a
                    key={item.href}
                    className={`${styles.navLink} ${
                      pathname === item.href ? styles.navLinkActive : ""
                    }`}
                    href={item.href}
                  >
                    {item.label}
                  </a>
                ))}
              </nav>
            </div>
            {!isHomePage && (
              <div className={styles.headerSearch}>
                {headerAccessory ?? <SearchBox variant="header" />}
              </div>
            )}
            <div className={styles.headerActions}>
              <div className={styles.mobileSearchRoot} ref={mobileSearchRef}>
                <button
                  type="button"
                  className={styles.headerIconButton}
                  aria-label={mobileSearchOpen ? "Close search" : "Open search"}
                  aria-controls="verifier-mobile-search"
                  aria-expanded={mobileSearchOpen}
                  onClick={() =>
                    setMobileHeaderPanel(current => (current === "search" ? undefined : "search"))
                  }
                >
                  {mobileSearchOpen ? <X size={18} /> : <Search size={18} />}
                </button>
                {mobileSearchOpen && (
                  <div id="verifier-mobile-search" className={styles.mobileSearchPanel}>
                    <SearchBox autoFocus className={styles.mobileSearch} variant="header" />
                  </div>
                )}
              </div>
              <span className={styles.desktopHeaderAction}>
                <ThemeSwitch />
              </span>
              <a
                className={`${styles.githubButton} ${styles.desktopHeaderAction}`}
                href="https://github.com/ton-blockchain/acton"
                target="_blank"
                rel="noreferrer"
                aria-label="Open GitHub"
                title="GitHub"
              >
                <Github size={18} strokeWidth={2} aria-hidden="true" />
              </a>
              <div className={styles.mobileNavigationRoot} ref={mobileNavigationRef}>
                <button
                  type="button"
                  className={styles.headerIconButton}
                  aria-label={mobileNavigationOpen ? "Close navigation" : "Open navigation"}
                  aria-controls="verifier-mobile-navigation"
                  aria-expanded={mobileNavigationOpen}
                  onClick={() =>
                    setMobileHeaderPanel(current =>
                      current === "navigation" ? undefined : "navigation",
                    )
                  }
                >
                  {mobileNavigationOpen ? <X size={18} /> : <Menu size={18} />}
                </button>
                {mobileNavigationOpen && (
                  <nav
                    id="verifier-mobile-navigation"
                    className={styles.mobileNavigation}
                    aria-label="Mobile TON Verifier navigation"
                  >
                    {PRIMARY_NAV_ITEMS.map(item => (
                      <a key={item.href} href={item.href}>
                        {item.label}
                      </a>
                    ))}
                    <a
                      href="https://github.com/ton-blockchain/acton"
                      target="_blank"
                      rel="noreferrer"
                    >
                      <span>GitHub</span>
                      <Github size={17} aria-hidden="true" />
                    </a>
                    <div className={styles.mobileThemeRow}>
                      <span>Appearance</span>
                      <ThemeSwitch />
                    </div>
                  </nav>
                )}
              </div>
            </div>
          </div>
        </header>
        <main className={styles.main}>{children}</main>
      </div>
    </ThemeProvider>
  )
}
