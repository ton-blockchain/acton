import type * as React from "react"
import {PanelLeft} from "lucide-react"
import {useCallback, useEffect, useRef, useState} from "react"
import {FiWifiOff} from "react-icons/fi"

import type {ThemeMode} from "@acton/ui"
import type {TestReport} from "@acton/shared-ui"

import styles from "./App.module.css"
import {Coverage} from "./components/Coverage/Coverage"
import {GasProfile} from "./components/GasProfile/GasProfile"
import {Sidebar} from "./components/Sidebar/Sidebar"
import {TestDetails} from "./components/TestDetails/TestDetails"
import {useRunnerConnection} from "./hooks/useRunnerConnection"
import {useTestTrace} from "./hooks/useTestTrace"
import {useTestUiBootstrap} from "./hooks/useTestUiBootstrap"

const SIDEBAR_TRANSITION_MS = 250

type ActiveView = "tests" | "coverage" | "profile"

const readInitialTheme = (): ThemeMode => {
  const storedTheme = localStorage.getItem("theme")
  if (storedTheme === "dark" || storedTheme === "light") {
    return storedTheme
  }

  return globalThis.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light"
}

export const App: React.FC = () => {
  const {connectionLost, markConnected} = useRunnerConnection()
  const {
    reports,
    reportsLoading: loading,
    projectRoot,
    capabilitiesLoaded,
    coverageAvailable,
    gasProfileAvailable,
  } = useTestUiBootstrap(markConnected)
  const [selectedTest, setSelectedTest] = useState<TestReport | undefined>()
  const {
    trace: currentTrace,
    error: currentTraceError,
    loading: isCurrentTraceLoading,
  } = useTestTrace(selectedTest)
  const [theme, setTheme] = useState<ThemeMode>(readInitialTheme)
  const [activeView, setActiveView] = useState<ActiveView>(() => {
    const saved = localStorage.getItem("activeMainView")
    return saved === "coverage" || saved === "profile" ? saved : "tests"
  })

  useEffect(() => {
    document.documentElement.classList.toggle("dark-theme", theme === "dark")
    localStorage.setItem("theme", theme)
  }, [theme])

  const toggleTheme = useCallback(() => {
    setTheme(prev => (prev === "light" ? "dark" : "light"))
  }, [])
  const handleActiveViewChange = useCallback((view: ActiveView) => {
    setActiveView(view)
    localStorage.setItem("activeMainView", view)
  }, [])
  const [sidebarWidth, setSidebarWidth] = useState(() => {
    const saved = localStorage.getItem("sidebarWidth")
    return saved ? Number.parseInt(saved, 10) : 350
  })
  const [isSidebarCollapsed, setIsSidebarCollapsed] = useState(() => {
    return localStorage.getItem("isSidebarCollapsed") === "true"
  })
  const [isSidebarPreviewOpen, setIsSidebarPreviewOpen] = useState(false)
  const [isSidebarPinningFromPreview, setIsSidebarPinningFromPreview] = useState(false)
  const [isSidebarResizing, setIsSidebarResizing] = useState(false)
  const [isSidebarClosing, setIsSidebarClosing] = useState(false)
  const [isHoveredResizer, setIsHoveredResizer] = useState(false)
  const isResizing = useRef(false)
  const lastWidth = useRef(sidebarWidth)
  const sidebarPinningTimeout = useRef<ReturnType<typeof globalThis.setTimeout> | undefined>(
    undefined,
  )
  const sidebarClosingTimeout = useRef<ReturnType<typeof globalThis.setTimeout> | undefined>(
    undefined,
  )
  const handleSelectTest = useCallback((test: TestReport) => {
    setSelectedTest(test)
    localStorage.setItem("selectedTest", `${test.suite_name}::${test.name}`)
  }, [])

  const handleMouseMove = useCallback((e: MouseEvent) => {
    if (!isResizing.current) return
    const newWidth = Math.max(200, Math.min(800, e.clientX))
    setSidebarWidth(newWidth)
    localStorage.setItem("sidebarWidth", newWidth.toString())
    lastWidth.current = newWidth
  }, [])

  const stopResizing = useCallback(() => {
    isResizing.current = false
    setIsSidebarResizing(false)
    document.removeEventListener("mousemove", handleMouseMove)
    document.removeEventListener("mouseup", stopResizing)
    document.body.style.cursor = ""
    document.body.style.userSelect = ""
  }, [handleMouseMove])

  const startResizing = useCallback(() => {
    if (isSidebarCollapsed) return
    isResizing.current = true
    setIsSidebarResizing(true)
    document.addEventListener("mousemove", handleMouseMove)
    document.addEventListener("mouseup", stopResizing)
    document.body.style.cursor = "col-resize"
    document.body.style.userSelect = "none"
  }, [handleMouseMove, stopResizing, isSidebarCollapsed])

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
    localStorage.setItem("isSidebarCollapsed", "true")
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
    localStorage.setItem("isSidebarCollapsed", "false")
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

  useEffect(() => {
    if (reports.length === 0) {
      return
    }

    const selectedTestExists = reports.some(
      report =>
        report.name === selectedTest?.name && report.suite_name === selectedTest?.suite_name,
    )
    if (selectedTestExists) {
      return
    }

    const savedTestId = localStorage.getItem("selectedTest")
    const savedTest = reports.find(report => `${report.suite_name}::${report.name}` === savedTestId)
    handleSelectTest(savedTest ?? reports[0])
  }, [handleSelectTest, reports, selectedTest])

  useEffect(() => {
    if (capabilitiesLoaded && !coverageAvailable && activeView === "coverage") {
      handleActiveViewChange("tests")
    }
    if (capabilitiesLoaded && !gasProfileAvailable && activeView === "profile") {
      handleActiveViewChange("tests")
    }
  }, [
    activeView,
    capabilitiesLoaded,
    coverageAvailable,
    gasProfileAvailable,
    handleActiveViewChange,
  ])

  if (loading && reports.length === 0) {
    return <div className={styles.loadingContainer}>Loading...</div>
  }

  const sidebarSlotStyle = {
    "--sidebar-expanded-width": `${sidebarWidth}px`,
    width: isSidebarCollapsed ? 0 : sidebarWidth,
  } as React.CSSProperties
  const isSidebarFloating = isSidebarCollapsed && isSidebarPreviewOpen

  return (
    <div className={styles.app}>
      {connectionLost && (
        <div className={styles.connectionOverlay}>
          <div
            className={styles.connectionDialog}
            role="alertdialog"
            aria-modal="true"
            aria-labelledby="connection-lost-title"
            aria-describedby="connection-lost-description"
          >
            <div className={styles.connectionIcon} aria-hidden="true">
              <FiWifiOff />
            </div>
            <h1 id="connection-lost-title" className={styles.connectionTitle}>
              Connection lost
            </h1>
            <p id="connection-lost-description" className={styles.connectionMessage}>
              The connection to the test runner was lost. Restart the runner to continue using the
              test UI.
            </p>
          </div>
        </div>
      )}

      <div
        className={[
          styles.sidebarSlot,
          isSidebarCollapsed ? styles.sidebarSlotCollapsed : "",
          isSidebarFloating ? styles.sidebarSlotFloating : "",
          isSidebarPinningFromPreview ? styles.sidebarSlotPinning : "",
          isSidebarResizing ? styles.sidebarSlotResizing : "",
          isSidebarClosing ? styles.sidebarSlotClosing : "",
        ].join(" ")}
        style={sidebarSlotStyle}
        aria-hidden={isSidebarCollapsed && !isSidebarPreviewOpen}
        data-testid="sidebar-slot"
      >
        {isSidebarCollapsed && (
          <div
            className={styles.sidebarPeekTarget}
            onPointerEnter={showSidebarPreview}
            aria-hidden="true"
            data-testid="sidebar-peek-target"
          />
        )}
        <div className={styles.sidebarViewport} onPointerLeave={hideSidebarPreview}>
          <Sidebar
            reports={reports}
            selectedTest={selectedTest}
            onSelectTest={handleSelectTest}
            width={sidebarWidth}
            onCollapse={toggleSidebar}
            isCollapsed={isSidebarCollapsed}
            className={styles.floatingSidebar}
            theme={theme}
            onToggleTheme={toggleTheme}
          />
        </div>
      </div>

      {isSidebarCollapsed && (activeView !== "tests" || !selectedTest) && (
        <button
          type="button"
          onClick={expandSidebar}
          className={styles.expandButton}
          aria-label="Expand sidebar"
          title="Expand sidebar"
        >
          <PanelLeft aria-hidden="true" />
        </button>
      )}

      {/* eslint-disable-next-line jsx-a11y/no-noninteractive-element-interactions */}
      <div
        onMouseDown={startResizing}
        onMouseEnter={() => setIsHoveredResizer(true)}
        onMouseLeave={() => setIsHoveredResizer(false)}
        role="separator"
        aria-valuenow={isSidebarCollapsed ? 0 : sidebarWidth}
        aria-valuemin={200}
        aria-valuemax={800}
        aria-label="Resize sidebar"
        className={`${styles.resizer} ${isSidebarCollapsed ? "" : styles.resizerActive} ${
          isHoveredResizer && !isSidebarCollapsed ? styles.resizerHovered : ""
        }`}
      />

      <div className={styles.mainContent}>
        {(coverageAvailable || gasProfileAvailable) && (
          <div className={styles.viewTabs} role="tablist" aria-label="Main view">
            <button
              type="button"
              role="tab"
              aria-selected={activeView === "tests"}
              className={`${styles.viewTab} ${activeView === "tests" ? styles.viewTabActive : ""}`}
              onClick={() => handleActiveViewChange("tests")}
            >
              Tests
            </button>
            {coverageAvailable && (
              <button
                type="button"
                role="tab"
                aria-selected={activeView === "coverage"}
                className={`${styles.viewTab} ${
                  activeView === "coverage" ? styles.viewTabActive : ""
                }`}
                onClick={() => handleActiveViewChange("coverage")}
              >
                Coverage
              </button>
            )}
            {gasProfileAvailable && (
              <button
                type="button"
                role="tab"
                aria-selected={activeView === "profile"}
                className={`${styles.viewTab} ${
                  activeView === "profile" ? styles.viewTabActive : ""
                }`}
                onClick={() => handleActiveViewChange("profile")}
              >
                Profile
              </button>
            )}
          </div>
        )}

        <div className={styles.mainPanel}>
          {activeView === "profile" && gasProfileAvailable ? (
            <div className={styles.profileView}>
              <GasProfile projectRoot={projectRoot} />
            </div>
          ) : activeView === "coverage" && coverageAvailable ? (
            <Coverage projectRoot={projectRoot} />
          ) : selectedTest ? (
            <TestDetails
              test={selectedTest}
              trace={currentTrace}
              traceError={currentTraceError}
              isTraceLoading={isCurrentTraceLoading}
              projectRoot={projectRoot}
              gasProfileAvailable={gasProfileAvailable}
              gasProfileAvailabilityLoaded={capabilitiesLoaded}
              isSidebarCollapsed={isSidebarCollapsed}
              onExpandSidebar={expandSidebar}
            />
          ) : (
            <div className={styles.noSelection}>Select a test to see details</div>
          )}
        </div>
      </div>
    </div>
  )
}
