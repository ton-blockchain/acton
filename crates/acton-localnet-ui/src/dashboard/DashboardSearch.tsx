import {Search} from "lucide-react"
import * as React from "react"

import type {TonClient} from "../explorer/api/client"

import {isTextEntryTarget} from "./dashboardUtils"
import styles from "./DashboardPage.module.css"

interface DashboardSearchProps {
  readonly client: TonClient
}

type SearchOriginStyle = Readonly<React.CSSProperties> & {
  readonly "--search-origin-left"?: string
  readonly "--search-origin-top"?: string
  readonly "--search-origin-width"?: string
  readonly "--search-origin-height"?: string
}

const DashboardSearchOverlay = React.lazy(async () => {
  const module = await import("./DashboardSearchOverlay")
  return {default: module.DashboardSearchOverlay}
})

export const DashboardSearch: React.FC<DashboardSearchProps> = ({client}) => {
  const [isSearchMounted, setIsSearchMounted] = React.useState(false)
  const [isSearchOpen, setIsSearchOpen] = React.useState(false)
  const [searchOriginStyle, setSearchOriginStyle] = React.useState<SearchOriginStyle>({})
  const searchButtonRef = React.useRef<HTMLButtonElement>(null)
  const searchAnimationRef = React.useRef<number | undefined>(undefined)

  const measureSearchOrigin = React.useCallback(() => {
    const searchButton = searchButtonRef.current
    const rect = searchButton?.getBoundingClientRect()
    if (!rect) {
      return
    }

    let originLeft = rect.left
    let originTop = rect.top
    const sidebar = searchButton?.closest<HTMLElement>(`.${styles.sidebar}`)
    if (sidebar && globalThis.matchMedia("(max-width: 920px)").matches) {
      const sidebarRect = sidebar.getBoundingClientRect()
      originLeft = rect.left - sidebarRect.left
      originTop = rect.top - sidebarRect.top
    }

    setSearchOriginStyle({
      "--search-origin-left": `${originLeft}px`,
      "--search-origin-top": `${originTop}px`,
      "--search-origin-width": `${rect.width}px`,
      "--search-origin-height": `${rect.height}px`,
    })
  }, [])

  const openSearch = React.useCallback(() => {
    measureSearchOrigin()
    setIsSearchMounted(true)
    if (searchAnimationRef.current !== undefined) {
      cancelAnimationFrame(searchAnimationRef.current)
    }
    searchAnimationRef.current = requestAnimationFrame(() => {
      setIsSearchOpen(true)
    })
  }, [measureSearchOrigin])

  const closeSearch = React.useCallback(() => {
    setIsSearchOpen(false)
  }, [])

  const handleGlobalKeyDown = React.useCallback(
    (event: KeyboardEvent) => {
      if (event.defaultPrevented || event.metaKey || event.ctrlKey || event.altKey) {
        return
      }

      if (event.key === "Escape" && isSearchMounted) {
        event.preventDefault()
        closeSearch()
        return
      }

      if (event.key.toLocaleLowerCase() !== "f" || isTextEntryTarget(event.target)) {
        return
      }

      event.preventDefault()
      openSearch()
    },
    [closeSearch, isSearchMounted, openSearch],
  )

  React.useEffect(() => {
    return () => {
      if (searchAnimationRef.current !== undefined) {
        cancelAnimationFrame(searchAnimationRef.current)
      }
    }
  }, [])

  React.useEffect(() => {
    globalThis.addEventListener("keydown", handleGlobalKeyDown)
    return () => {
      globalThis.removeEventListener("keydown", handleGlobalKeyDown)
    }
  }, [handleGlobalKeyDown])

  React.useEffect(() => {
    if (!isSearchMounted) {
      return
    }

    const handleResize = () => measureSearchOrigin()
    globalThis.addEventListener("resize", handleResize)
    return () => {
      globalThis.removeEventListener("resize", handleResize)
    }
  }, [isSearchMounted, measureSearchOrigin])

  React.useEffect(() => {
    if (!isSearchMounted || isSearchOpen) {
      return
    }

    const timeout = globalThis.setTimeout(() => {
      setIsSearchMounted(false)
    }, 300)

    return () => {
      globalThis.clearTimeout(timeout)
    }
  }, [isSearchMounted, isSearchOpen])

  return (
    <>
      <button
        ref={searchButtonRef}
        type="button"
        className={`${styles.searchButton} ${isSearchMounted ? styles.searchButtonMorphing : ""}`}
        onClick={openSearch}
      >
        <span className={styles.searchButtonValue}>
          <Search size={16} />
          <span>Find...</span>
        </span>
        <span className={styles.searchShortcut}>F</span>
      </button>

      {isSearchMounted ? (
        <React.Suspense
          fallback={
            <SearchOverlayFallback
              isOpen={isSearchOpen}
              style={searchOriginStyle}
              onClose={closeSearch}
            />
          }
        >
          <DashboardSearchOverlay
            client={client}
            isOpen={isSearchOpen}
            onClose={closeSearch}
            originStyle={searchOriginStyle}
          />
        </React.Suspense>
      ) : undefined}
    </>
  )
}

const SearchOverlayFallback: React.FC<{
  readonly isOpen: boolean
  readonly onClose: () => void
  readonly style: SearchOriginStyle
}> = ({isOpen, onClose, style}) => (
  <div
    className={`${styles.searchOverlay} ${isOpen ? styles.searchOverlayOpen : ""}`}
    aria-hidden={!isOpen}
    style={style}
  >
    <button
      type="button"
      className={styles.searchBackdrop}
      aria-label="Close search"
      onClick={onClose}
    />
    <section className={styles.searchPanel} role="dialog" aria-modal="true" aria-label="Search">
      <div className={styles.searchInputRow}>
        <Search size={17} className={styles.searchInputIcon} />
        <div className={styles.searchInput}>Loading search...</div>
      </div>
    </section>
  </div>
)
