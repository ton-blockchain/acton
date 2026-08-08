import {Search} from "lucide-react"
import {createPortal} from "react-dom"
import {useCallback, useEffect, useRef, useState} from "react"
import type {CSSProperties, FC} from "react"

import type {TonClient} from "@acton/explorer-core/api/client"

import {isTextEntryTarget} from "./dashboardUtils"
import styles from "./DashboardPage.module.css"
import {DashboardSearchOverlay} from "./DashboardSearchOverlay"

interface DashboardSearchProps {
  readonly client: TonClient
}

type SearchOriginStyle = Readonly<CSSProperties> & {
  readonly "--search-origin-left"?: string
  readonly "--search-origin-top"?: string
  readonly "--search-origin-width"?: string
  readonly "--search-origin-height"?: string
}

export const DashboardSearch: FC<DashboardSearchProps> = ({client}) => {
  const [isSearchMounted, setIsSearchMounted] = useState(false)
  const [isSearchOpen, setIsSearchOpen] = useState(false)
  const [searchOriginStyle, setSearchOriginStyle] = useState<SearchOriginStyle>({})
  const searchButtonRef = useRef<HTMLButtonElement>(null)
  const searchAnimationRef = useRef<number | undefined>(undefined)

  const measureSearchOrigin = useCallback(() => {
    const searchButton = searchButtonRef.current
    const rect = searchButton?.getBoundingClientRect()
    if (!rect) {
      return
    }

    setSearchOriginStyle({
      "--search-origin-left": `${rect.left}px`,
      "--search-origin-top": `${rect.top}px`,
      "--search-origin-width": `${rect.width}px`,
      "--search-origin-height": `${rect.height}px`,
    })
  }, [])

  const openSearch = useCallback(() => {
    measureSearchOrigin()
    setIsSearchMounted(true)
    if (searchAnimationRef.current !== undefined) {
      cancelAnimationFrame(searchAnimationRef.current)
    }
    searchAnimationRef.current = requestAnimationFrame(() => {
      setIsSearchOpen(true)
    })
  }, [measureSearchOrigin])

  const closeSearch = useCallback(() => {
    setIsSearchOpen(false)
  }, [])

  const handleGlobalKeyDown = useCallback(
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

  useEffect(() => {
    return () => {
      if (searchAnimationRef.current !== undefined) {
        cancelAnimationFrame(searchAnimationRef.current)
      }
    }
  }, [])

  useEffect(() => {
    globalThis.addEventListener("keydown", handleGlobalKeyDown)
    return () => {
      globalThis.removeEventListener("keydown", handleGlobalKeyDown)
    }
  }, [handleGlobalKeyDown])

  useEffect(() => {
    if (!isSearchMounted) {
      return
    }

    const handleResize = () => measureSearchOrigin()
    globalThis.addEventListener("resize", handleResize)
    return () => {
      globalThis.removeEventListener("resize", handleResize)
    }
  }, [isSearchMounted, measureSearchOrigin])

  useEffect(() => {
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

      {isSearchMounted
        ? createPortal(
            <DashboardSearchOverlay
              client={client}
              isOpen={isSearchOpen}
              onClose={closeSearch}
              originStyle={searchOriginStyle}
            />,
            document.body,
          )
        : undefined}
    </>
  )
}
