import {CircleHelp, PanelLeftOpen} from "lucide-react"
import {useCallback, useEffect, useRef, useState} from "react"
import type {ReactNode} from "react"
import {Tooltip} from "@acton/ui"

import type {StudioPage, StudioPath} from "../studioPages"
import {StudioNavigation} from "./StudioNavigation"

import styles from "./StudioNavigation.module.css"

const SIDEBAR_TRANSITION_MS = 250
const SIDEBAR_COLLAPSED_STORAGE_KEY = "studioSidebarCollapsed"

interface StudioShellProps {
  readonly activePath: StudioPath
  readonly children: ReactNode
  readonly headerActions?: ReactNode
  readonly pages: readonly StudioPage[]
  readonly projectName?: string
  readonly projectPath?: string
  readonly onNavigate: (path: StudioPath) => void
}

export function StudioShell({
  activePath,
  children,
  headerActions,
  pages,
  projectName,
  projectPath,
  onNavigate,
}: StudioShellProps) {
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
    if (sidebarPinningTimeout.current === undefined) return

    globalThis.clearTimeout(sidebarPinningTimeout.current)
    sidebarPinningTimeout.current = undefined
  }, [])

  const clearSidebarClosingTimeout = useCallback(() => {
    if (sidebarClosingTimeout.current === undefined) return

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
    if (isSidebarCollapsed) setIsSidebarPreviewOpen(true)
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
  const activePage = pages.find(page => page.path === activePath) ?? pages[0]

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
          <StudioNavigation
            activePath={activePath}
            className={styles.floatingSidebar}
            isSidebarCollapsed={isSidebarCollapsed}
            pages={pages}
            projectName={projectName}
            projectPath={projectPath}
            onNavigate={onNavigate}
            onToggleSidebar={toggleSidebar}
          />
        </div>
      </div>

      {isSidebarCollapsed && !isSidebarPreviewOpen && (
        <Tooltip content="Expand navigation">
          <button
            type="button"
            className={styles.sidebarExpandButton}
            aria-label="Expand navigation"
            onClick={expandSidebar}
          >
            <PanelLeftOpen size={18} />
          </button>
        </Tooltip>
      )}

      <section className={styles.contentArea}>
        <header className={styles.pageHeader}>
          <div className={styles.pageHeaderInner}>
            <div className={styles.pageHeaderTitleGroup}>
              <span className={styles.pageHeaderTitle}>{activePage.label}</span>
              <Tooltip content={activePage.shortDescription}>
                <button
                  type="button"
                  className={styles.pageHeaderHelp}
                  aria-label={`About ${activePage.label}`}
                >
                  <CircleHelp size={15} />
                </button>
              </Tooltip>
            </div>
            {headerActions && <div className={styles.pageHeaderActions}>{headerActions}</div>}
          </div>
        </header>
        <main className={styles.content}>{children}</main>
      </section>
    </div>
  )
}
