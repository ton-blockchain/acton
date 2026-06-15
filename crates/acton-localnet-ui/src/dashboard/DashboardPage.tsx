import type React from "react"
import {PanelLeftOpen} from "lucide-react"
import {useCallback, useEffect, useRef, useState} from "react"
import type {ThemeMode} from "@acton/shared-ui"

import type {TonClient} from "../explorer/api/client"

import {DashboardNavigation} from "./DashboardNavigation"
import styles from "./DashboardPage.module.css"

const SIDEBAR_TRANSITION_MS = 250
const SIDEBAR_COLLAPSED_STORAGE_KEY = "localnetSidebarCollapsed"

interface DashboardPageProps {
  readonly client: TonClient
  readonly localnetApiToken?: string
  readonly onOpenAuthTokenOverlay: () => void
  readonly theme: ThemeMode
  readonly setTheme: (theme: ThemeMode) => void
  readonly children?: React.ReactNode
  readonly embedded?: boolean
}

export const DashboardPage: React.FC<DashboardPageProps> = ({
  children,
  client,
  embedded = false,
  localnetApiToken,
  onOpenAuthTokenOverlay,
  theme,
  setTheme,
}) => {
  const [isSidebarCollapsed, setIsSidebarCollapsed] = useState(() => {
    return localStorage.getItem(SIDEBAR_COLLAPSED_STORAGE_KEY) === "true"
  })
  const [isSidebarPreviewOpen, setIsSidebarPreviewOpen] = useState(false)
  const [isSidebarPinningFromPreview, setIsSidebarPinningFromPreview] = useState(false)
  const [isSidebarClosing, setIsSidebarClosing] = useState(false)
  const sidebarPinningTimeout = useRef<ReturnType<typeof globalThis.setTimeout> | undefined>(
    undefined,
  )
  const sidebarClosingTimeout = useRef<ReturnType<typeof globalThis.setTimeout> | undefined>(
    undefined,
  )

  const clearSidebarPinningTimeout = useCallback(() => {
    if (sidebarPinningTimeout.current === undefined) {
      return
    }

    globalThis.clearTimeout(sidebarPinningTimeout.current)
    sidebarPinningTimeout.current = undefined
  }, [])

  const clearSidebarClosingTimeout = useCallback(() => {
    if (sidebarClosingTimeout.current === undefined) {
      return
    }

    globalThis.clearTimeout(sidebarClosingTimeout.current)
    sidebarClosingTimeout.current = undefined
  }, [])

  const finishSidebarPinning = useCallback(() => {
    clearSidebarPinningTimeout()
    setIsSidebarPinningFromPreview(false)
  }, [clearSidebarPinningTimeout])

  const finishSidebarClosing = useCallback(() => {
    clearSidebarClosingTimeout()
    setIsSidebarClosing(false)
  }, [clearSidebarClosingTimeout])

  const startSidebarPinning = useCallback(() => {
    clearSidebarPinningTimeout()
    setIsSidebarPinningFromPreview(true)
    sidebarPinningTimeout.current = globalThis.setTimeout(
      finishSidebarPinning,
      SIDEBAR_TRANSITION_MS,
    )
  }, [clearSidebarPinningTimeout, finishSidebarPinning])

  const startSidebarClosing = useCallback(() => {
    clearSidebarClosingTimeout()
    setIsSidebarClosing(true)
    sidebarClosingTimeout.current = globalThis.setTimeout(
      finishSidebarClosing,
      SIDEBAR_TRANSITION_MS,
    )
  }, [clearSidebarClosingTimeout, finishSidebarClosing])

  const collapseSidebar = useCallback(() => {
    clearSidebarPinningTimeout()
    setIsSidebarPinningFromPreview(false)
    setIsSidebarPreviewOpen(false)
    startSidebarClosing()
    setIsSidebarCollapsed(true)
    localStorage.setItem(SIDEBAR_COLLAPSED_STORAGE_KEY, "true")
  }, [clearSidebarPinningTimeout, startSidebarClosing])

  const expandSidebar = useCallback(() => {
    clearSidebarClosingTimeout()
    setIsSidebarClosing(false)

    if (isSidebarCollapsed && isSidebarPreviewOpen) {
      startSidebarPinning()
    } else {
      clearSidebarPinningTimeout()
      setIsSidebarPinningFromPreview(false)
    }

    setIsSidebarPreviewOpen(false)
    setIsSidebarCollapsed(false)
    localStorage.setItem(SIDEBAR_COLLAPSED_STORAGE_KEY, "false")
  }, [
    clearSidebarClosingTimeout,
    clearSidebarPinningTimeout,
    isSidebarCollapsed,
    isSidebarPreviewOpen,
    startSidebarPinning,
  ])

  const toggleSidebar = useCallback(() => {
    if (isSidebarCollapsed) {
      expandSidebar()
    } else {
      collapseSidebar()
    }
  }, [collapseSidebar, expandSidebar, isSidebarCollapsed])

  const showSidebarPreview = useCallback(() => {
    if (isSidebarCollapsed) {
      setIsSidebarPreviewOpen(true)
    }
  }, [isSidebarCollapsed])

  const hideSidebarPreview = useCallback(() => {
    setIsSidebarPreviewOpen(false)
  }, [])

  useEffect(() => {
    return () => {
      clearSidebarPinningTimeout()
      clearSidebarClosingTimeout()
    }
  }, [clearSidebarClosingTimeout, clearSidebarPinningTimeout])

  const isSidebarFloating = isSidebarCollapsed && isSidebarPreviewOpen

  return (
    <div className={styles.page}>
      <div
        className={[
          styles.sidebarSlot,
          isSidebarCollapsed ? styles.sidebarSlotCollapsed : "",
          isSidebarFloating ? styles.sidebarSlotFloating : "",
          isSidebarPinningFromPreview ? styles.sidebarSlotPinning : "",
          isSidebarClosing ? styles.sidebarSlotClosing : "",
        ].join(" ")}
      >
        {isSidebarCollapsed && (
          <div
            className={styles.sidebarPeekTarget}
            onPointerEnter={showSidebarPreview}
            aria-hidden="true"
          />
        )}
        <div className={styles.sidebarViewport} onPointerLeave={hideSidebarPreview}>
          <DashboardNavigation
            client={client}
            localnetApiToken={localnetApiToken}
            onOpenAuthTokenOverlay={onOpenAuthTokenOverlay}
            theme={theme}
            setTheme={setTheme}
            onToggleSidebar={toggleSidebar}
            isSidebarCollapsed={isSidebarCollapsed}
            className={styles.floatingSidebar}
          />
        </div>
      </div>

      {isSidebarCollapsed && !isSidebarPreviewOpen && (
        <button
          type="button"
          onClick={expandSidebar}
          className={styles.sidebarExpandButton}
          title="Expand navigation"
          aria-label="Expand navigation"
        >
          <PanelLeftOpen size={18} />
        </button>
      )}

      <section className={styles.contentArea}>
        <main className={`${styles.content} ${embedded ? styles.contentEmbedded : ""}`}>
          {embedded ? <div className={styles.embeddedPage}>{children}</div> : children}
        </main>
      </section>
    </div>
  )
}
