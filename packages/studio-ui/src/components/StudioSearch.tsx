import {Search} from "lucide-react"
import {Dialog, InlineLoader} from "@acton/ui"
import {Suspense, lazy, useCallback, useEffect, useRef, useState} from "react"

import type {StudioEnvironment} from "../studioApi"

import navigationStyles from "./StudioNavigation.module.css"
import styles from "./StudioSearch.module.css"

interface StudioSearchProps {
  readonly environments: readonly StudioEnvironment[]
  readonly onNavigate: (path: string) => void
}

const StudioSearchOverlay = lazy(async () => {
  const module = await import("./StudioSearchOverlay")
  return {default: module.StudioSearchOverlay}
})

/** One search entry point throughout Studio; Dialog owns modal focus, dismissal and restoration */
export function StudioSearch({environments, onNavigate}: StudioSearchProps) {
  const [open, setOpen] = useState(false)
  const [session, setSession] = useState(0)
  const triggerRef = useRef<HTMLButtonElement>(null)
  const shortcut = /Mac|iPhone|iPad/.test(navigator.userAgent) ? "⌘ K" : "Ctrl K"

  const openSearch = useCallback(() => {
    setSession(value => value + 1)
    setOpen(true)
  }, [])

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.defaultPrevented || event.isComposing || event.altKey) return

      const target = event.target
      const typing =
        target instanceof HTMLElement &&
        (target.matches("input, textarea, select") || target.isContentEditable)
      const command = (event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "k"
      const find = !typing && !event.metaKey && !event.ctrlKey && event.key.toLowerCase() === "f"
      if (!command && !find) return

      // Do not steal shortcuts from another dialog or a composing text input.
      if (document.querySelector('[role="dialog"][aria-modal="true"]') && !open) return
      event.preventDefault()
      if (!open) openSearch()
    }

    globalThis.addEventListener("keydown", handleKeyDown)
    return () => globalThis.removeEventListener("keydown", handleKeyDown)
  }, [open, openSearch])

  return (
    <>
      <button
        ref={triggerRef}
        type="button"
        className={navigationStyles.searchButton}
        aria-label="Search Studio"
        aria-haspopup="dialog"
        aria-expanded={open}
        onClick={openSearch}
      >
        <span className={navigationStyles.searchButtonValue}>
          <Search size={16} aria-hidden="true" />
          <span>Search</span>
        </span>
        <kbd className={styles.shortcut}>{shortcut}</kbd>
      </button>

      <Dialog
        open={open}
        onOpenChange={setOpen}
        onOpenChangeComplete={nextOpen => {
          if (!nextOpen) triggerRef.current?.focus()
        }}
        title="Search Studio"
        closeLabel="Close search"
        maxWidth="40rem"
        contentClassName={styles.content}
        footer={
          <div className={styles.footer}>
            <span>
              <kbd>↑</kbd> <kbd>↓</kbd> Navigate
            </span>
            <span>
              <kbd>↵</kbd> Open
            </span>
            <span>
              <kbd>Esc</kbd> Close
            </span>
          </div>
        }
      >
        <Suspense
          fallback={
            <div className={styles.loading}>
              <InlineLoader message="Loading search" />
            </div>
          }
        >
          <StudioSearchOverlay
            key={session}
            environments={environments}
            open={open}
            onSelect={path => {
              setOpen(false)
              onNavigate(path)
            }}
          />
        </Suspense>
      </Dialog>
    </>
  )
}
