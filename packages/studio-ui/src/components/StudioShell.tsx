import {CircleHelp, PanelLeftOpen} from "lucide-react"
import {useCallback, useEffect, useRef, useState} from "react"
import type {ReactNode} from "react"
import {Tooltip} from "@acton/ui"

import type {StudioEnvironment, TestRunSummary} from "../studioApi"
import type {StudioPage, StudioPath} from "../studioPages"
import {StudioNavigation} from "./StudioNavigation"

import styles from "./StudioNavigation.module.css"

const SIDEBAR_TRANSITION_MS = 250
const SIDEBAR_COLLAPSED_STORAGE_KEY = "studioSidebarCollapsed"

interface StudioShellProps {
  readonly activePath: StudioPath
  readonly children: ReactNode
  readonly contentMode?: "default" | "full" | "workspace"
  readonly headerMode?: "visible" | "hidden"
  readonly headerActions?: ReactNode
  readonly pageDescription?: string
  readonly pageTitle?: string
  readonly pages: readonly StudioPage[]
  readonly sidebarActiveEnvironmentId?: string
  readonly sidebarActiveNetworkId?: string
  readonly sidebarContextAction?: {
    readonly label: string
    readonly onSelect: () => void
  }
  readonly sidebarEnvironments?: readonly StudioEnvironment[]
  readonly sidebarNetworks?: readonly StudioEnvironment[]
  readonly sidebarNavigation?: ReactNode
  readonly sidebarNavigationKey?: string
  readonly sidebarSearch?: ReactNode
  readonly sidebarSelectedTestRunId?: string
  readonly sidebarTestRuns?: readonly TestRunSummary[]
  readonly sidebarUtilityActions?: ReactNode
  readonly onNavigate: (path: StudioPath) => void
  readonly onOpenEnvironment?: (environment: StudioEnvironment) => void
  readonly onSelectTestRun?: (runId: string) => void
}

export function StudioShell({
  activePath,
  children,
  contentMode = "default",
  headerMode = "visible",
  headerActions,
  pageDescription,
  pageTitle,
  pages,
  sidebarActiveEnvironmentId,
  sidebarActiveNetworkId,
  sidebarContextAction,
  sidebarEnvironments,
  sidebarNetworks,
  sidebarNavigation,
  sidebarNavigationKey,
  sidebarSearch,
  sidebarSelectedTestRunId,
  sidebarTestRuns,
  sidebarUtilityActions,
  onNavigate,
  onOpenEnvironment,
  onSelectTestRun,
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
  const activePageDescription = pageDescription ?? activePage.shortDescription
  const activePageTitle = pageTitle ?? activePage.label

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
            activeEnvironmentId={sidebarActiveEnvironmentId}
            activeNetworkId={sidebarActiveNetworkId}
            className={styles.floatingSidebar}
            contextAction={sidebarContextAction}
            environments={sidebarEnvironments}
            networks={sidebarNetworks}
            isSidebarCollapsed={isSidebarCollapsed}
            navigationContent={sidebarNavigation}
            navigationKey={sidebarNavigationKey}
            pages={pages}
            searchContent={sidebarSearch}
            selectedTestRunId={sidebarSelectedTestRunId}
            testRuns={sidebarTestRuns}
            utilityActions={sidebarUtilityActions}
            onNavigate={onNavigate}
            onOpenEnvironment={onOpenEnvironment}
            onSelectTestRun={onSelectTestRun}
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
        {headerMode === "visible" ? (
          <header className={styles.pageHeader}>
            <div className={styles.pageHeaderInner}>
              <div className={styles.pageHeaderTitleGroup}>
                <span className={styles.pageHeaderTitle}>{activePageTitle}</span>
                <Tooltip content={activePageDescription}>
                  <button
                    type="button"
                    className={styles.pageHeaderHelp}
                    aria-label={`About ${activePageTitle}`}
                  >
                    <CircleHelp size={15} />
                  </button>
                </Tooltip>
              </div>
              {headerActions && <div className={styles.pageHeaderActions}>{headerActions}</div>}
            </div>
          </header>
        ) : null}
        <div className={styles.contentViewport}>
          <main
            className={[
              styles.content,
              contentMode === "full" ? styles.contentFull : "",
              contentMode === "workspace" ? styles.contentWorkspace : "",
            ].join(" ")}
          >
            {children}
          </main>
        </div>
      </section>
    </div>
  )
}
