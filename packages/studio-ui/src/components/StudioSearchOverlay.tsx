import {Search} from "lucide-react"
import {useCallback, useEffect, useMemo, useRef, useState} from "react"
import type {KeyboardEvent as ReactKeyboardEvent} from "react"

import {studioPages, type StudioPath} from "../studioPages"
import type {SearchOriginStyle} from "./StudioSearch"

import styles from "./StudioNavigation.module.css"

interface StudioSearchOverlayProps {
  readonly isOpen: boolean
  readonly originStyle: SearchOriginStyle
  readonly onClose: () => void
  readonly onNavigate: (path: StudioPath) => void
}

export function StudioSearchOverlay({
  isOpen,
  originStyle,
  onClose,
  onNavigate,
}: StudioSearchOverlayProps) {
  const [searchQuery, setSearchQuery] = useState("")
  const searchInputRef = useRef<HTMLInputElement>(null)

  const searchResults = useMemo(() => {
    const query = searchQuery.trim().toLocaleLowerCase()
    if (!query) return studioPages

    return studioPages.filter(page => {
      return `${page.label} ${page.shortDescription}`.toLocaleLowerCase().includes(query)
    })
  }, [searchQuery])

  const selectSearchResult = useCallback(
    (path: StudioPath) => {
      onClose()
      setSearchQuery("")
      onNavigate(path)
    },
    [onClose, onNavigate],
  )

  const handleSearchKeyDown = useCallback(
    (event: ReactKeyboardEvent<HTMLInputElement>) => {
      if (event.key === "Escape") {
        event.preventDefault()
        onClose()
        return
      }

      if (event.key === "Enter" && searchResults[0]) {
        event.preventDefault()
        selectSearchResult(searchResults[0].path)
      }
    },
    [onClose, searchResults, selectSearchResult],
  )

  useEffect(() => {
    if (isOpen) searchInputRef.current?.focus()
  }, [isOpen])

  return (
    <div
      className={`${styles.searchOverlay} ${isOpen ? styles.searchOverlayOpen : ""}`}
      aria-hidden={!isOpen}
      style={originStyle}
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
          <input
            ref={searchInputRef}
            className={styles.searchInput}
            value={searchQuery}
            placeholder="Find..."
            autoComplete="off"
            autoCorrect="off"
            spellCheck={false}
            onChange={event => setSearchQuery(event.target.value)}
            onKeyDown={handleSearchKeyDown}
          />
          <button
            type="button"
            className={styles.searchEscButton}
            aria-label="Close search"
            onClick={onClose}
          >
            <span className={styles.searchEscShortcut}>F</span>
            <span className={styles.searchEscLabel}>Esc</span>
          </button>
        </div>

        <div className={styles.searchResultBody}>
          {searchResults.length === 0 ? (
            <div className={styles.searchEmpty}>No matching Studio pages.</div>
          ) : (
            <div className={styles.searchResultList}>
              {searchResults.map(result => {
                const Icon = result.icon

                return (
                  <button
                    key={result.path}
                    type="button"
                    className={styles.searchResultItem}
                    onClick={() => selectSearchResult(result.path)}
                  >
                    <span className={styles.searchResultIcon}>
                      {result.path === "/" ? (
                        <span className={styles.searchResultWorkspaceMark} />
                      ) : (
                        <Icon size={17} />
                      )}
                    </span>
                    <span className={styles.searchResultText}>
                      <span className={styles.searchResultTitle}>{result.label}</span>
                      <span className={styles.searchResultDescription}>
                        {result.shortDescription}
                      </span>
                    </span>
                  </button>
                )
              })}
            </div>
          )}
        </div>
      </section>
    </div>
  )
}
