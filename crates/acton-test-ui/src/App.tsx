import type * as React from "react"
import {useCallback, useEffect, useMemo, useRef, useState} from "react"
import {FiWifiOff} from "react-icons/fi"

import type {TestReport, ThemeMode, Trace} from "@acton/shared-ui"

import styles from "./App.module.css"
import {Coverage} from "./components/Coverage/Coverage"
import {GasProfile, type GasProfileReport} from "./components/GasProfile/GasProfile"
import {DocsSidebarIcon} from "./components/Sidebar/DocsSidebarIcon"
import {Sidebar} from "./components/Sidebar/Sidebar"
import {TestDetails} from "./components/TestDetails/TestDetails"

const RUNNER_HEALTH_POLL_INTERVAL_MS = 1500
const SIDEBAR_TRANSITION_MS = 250

type ActiveView = "tests" | "coverage" | "profile"

const readInitialTheme = (): ThemeMode => {
  const storedTheme = localStorage.getItem("theme")
  if (storedTheme === "dark" || storedTheme === "light") {
    return storedTheme
  }

  return globalThis.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light"
}

const formatResponseError = (response: Response, body: string): string => {
  const status = `${response.status} ${response.statusText}`.trim()
  const trimmedBody = body.trim()

  if (trimmedBody.length === 0) {
    return status
  }

  try {
    const json = JSON.parse(trimmedBody) as {error?: unknown}
    if (typeof json.error === "string" && json.error.trim().length > 0) {
      return `${status}: ${json.error}`
    }
  } catch {
    // Fall through to the raw response body below.
  }

  return `${status}: ${trimmedBody.slice(0, 500)}`
}

const parseTraceResponse = async (
  response: Response,
  tracePath: string,
): Promise<Trace | undefined> => {
  if (response.status === 204) {
    return undefined
  }

  const body = await response.text()

  if (!response.ok) {
    throw new Error(formatResponseError(response, body))
  }

  if (body.trim().length === 0) {
    return undefined
  }

  try {
    return JSON.parse(body) as Trace
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error)
    throw new Error(`Trace ${tracePath} is not valid JSON: ${reason}`)
  }
}

export const App: React.FC = () => {
  const [reports, setReports] = useState<TestReport[]>([])
  const [selectedTest, setSelectedTest] = useState<TestReport | undefined>()
  const [currentTrace, setCurrentTrace] = useState<Trace | undefined>()
  const [currentTraceError, setCurrentTraceError] = useState<string | undefined>()
  const [isCurrentTraceLoading, setIsCurrentTraceLoading] = useState(false)
  const [projectRoot, setProjectRoot] = useState<string>("")
  const [theme, setTheme] = useState<ThemeMode>(readInitialTheme)
  const [loading, setLoading] = useState(true)
  const [coverageLcov, setCoverageLcov] = useState<string | undefined>()
  const [coverageLoaded, setCoverageLoaded] = useState(false)
  const [gasProfile, setGasProfile] = useState<GasProfileReport | undefined>()
  const [gasProfileLoaded, setGasProfileLoaded] = useState(false)
  const [connectionLost, setConnectionLost] = useState(false)
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
  const hasConnectedToRunner = useRef(false)
  const traceFetchController = useRef<AbortController | undefined>(undefined)
  const traceFetchId = useRef(0)

  const markRunnerConnected = useCallback(() => {
    hasConnectedToRunner.current = true
    setConnectionLost(false)
  }, [])

  const handleSelectTest = useCallback((test: TestReport) => {
    traceFetchController.current?.abort()
    const fetchId = traceFetchId.current + 1
    traceFetchId.current = fetchId
    setSelectedTest(test)
    setCurrentTrace(undefined)
    setCurrentTraceError(undefined)
    localStorage.setItem("selectedTest", `${test.suite_name}::${test.name}`)
    if (test.trace_path) {
      setIsCurrentTraceLoading(true)
      const controller = new AbortController()
      traceFetchController.current = controller
      void fetch(`/api/trace/${encodeURIComponent(test.trace_path)}`, {signal: controller.signal})
        .then(res => parseTraceResponse(res, test.trace_path ?? "<unknown>"))
        .then(data => {
          if (traceFetchId.current !== fetchId) return
          setCurrentTrace(data)
          setCurrentTraceError(undefined)
        })
        .catch((error: unknown) => {
          if (error instanceof Error && error.name === "AbortError") {
            return
          }

          const message = error instanceof Error ? error.message : String(error)
          console.error("Failed to fetch trace", {
            suite: test.suite_name,
            test: test.name,
            tracePath: test.trace_path,
            error,
          })
          if (traceFetchId.current !== fetchId) return
          setCurrentTrace(undefined)
          setCurrentTraceError(message)
        })
        .finally(() => {
          if (traceFetchId.current === fetchId) {
            setIsCurrentTraceLoading(false)
          }
        })
    } else {
      setIsCurrentTraceLoading(false)
      setCurrentTrace(undefined)
      setCurrentTraceError(undefined)
    }
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
    const coverageController = new AbortController()
    const gasProfileController = new AbortController()
    const reportsController = new AbortController()
    const configController = new AbortController()

    void fetch("/api/config", {signal: configController.signal})
      .then(async res => (await res.json()) as {project_root: string})
      .then(data => {
        markRunnerConnected()
        setProjectRoot(data.project_root)
      })
      .catch(error => {
        if (error instanceof Error && error.name === "AbortError") {
          return
        }

        console.error("Failed to fetch config", error)
      })

    void fetch("/api/reports", {signal: reportsController.signal})
      .then(async res => (await res.json()) as TestReport[])
      .then(data => {
        markRunnerConnected()
        setReports(data)
        setLoading(false)
      })
      .catch(error => {
        if (error instanceof Error && error.name === "AbortError") {
          return
        }

        console.error("Failed to fetch reports", error)
        setLoading(false)
      })

    void fetch("/api/coverage.lcov", {signal: coverageController.signal})
      .then(async response => {
        if (response.status === 204) {
          markRunnerConnected()
          setCoverageLcov(undefined)
          setCoverageLoaded(true)
          return
        }

        if (!response.ok) {
          throw new Error(`Failed to fetch coverage report: ${response.status}`)
        }

        const lcov = await response.text()
        markRunnerConnected()
        setCoverageLcov(lcov)
        setCoverageLoaded(true)
      })
      .catch(error => {
        if (error instanceof Error && error.name === "AbortError") {
          return
        }

        console.error("Failed to fetch coverage report", error)
        setCoverageLcov(undefined)
        setCoverageLoaded(true)
      })

    void fetch("/api/gas-profile", {signal: gasProfileController.signal})
      .then(async response => {
        if (response.status === 204) {
          markRunnerConnected()
          setGasProfile(undefined)
          setGasProfileLoaded(true)
          return
        }

        if (!response.ok) {
          throw new Error(`Failed to fetch gas profile: ${response.status}`)
        }

        const profile = (await response.json()) as GasProfileReport
        markRunnerConnected()
        setGasProfile(profile)
        setGasProfileLoaded(true)
      })
      .catch(error => {
        if (error instanceof Error && error.name === "AbortError") {
          return
        }

        console.error("Failed to fetch gas profile", error)
        setGasProfile(undefined)
        setGasProfileLoaded(true)
      })

    return () => {
      coverageController.abort()
      gasProfileController.abort()
      reportsController.abort()
      configController.abort()
    }
  }, [markRunnerConnected])

  useEffect(() => {
    const checkRunnerConnection = () => {
      const controller = new AbortController()

      void fetch("/api/health", {
        cache: "no-store",
        signal: controller.signal,
      })
        .then(response => {
          if (!response.ok) {
            throw new Error(`Runner health check failed: ${response.status}`)
          }

          markRunnerConnected()
        })
        .catch(error => {
          if (error instanceof Error && error.name === "AbortError") {
            return
          }

          if (hasConnectedToRunner.current) {
            setConnectionLost(true)
          }
        })

      return controller
    }

    const currentController = checkRunnerConnection()
    const intervalId = globalThis.setInterval(() => {
      void checkRunnerConnection()
    }, RUNNER_HEALTH_POLL_INTERVAL_MS)

    return () => {
      currentController.abort()
      globalThis.clearInterval(intervalId)
    }
  }, [markRunnerConnected])

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
    if (coverageLoaded && coverageLcov === undefined && activeView === "coverage") {
      handleActiveViewChange("tests")
    }
    if (gasProfileLoaded && gasProfile === undefined && activeView === "profile") {
      handleActiveViewChange("tests")
    }
  }, [
    activeView,
    coverageLcov,
    coverageLoaded,
    gasProfile,
    gasProfileLoaded,
    handleActiveViewChange,
  ])

  const selectedTestGasProfile = useMemo(() => {
    if (selectedTest === undefined) {
      return
    }

    return gasProfile?.tests?.find(profile => profile.name === selectedTest.name)
  }, [gasProfile, selectedTest])

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
          <DocsSidebarIcon />
        </button>
      )}

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
        {(coverageLcov !== undefined || gasProfile !== undefined) && (
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
            {coverageLcov !== undefined && (
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
            {gasProfile !== undefined && (
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
          {activeView === "profile" && gasProfile !== undefined ? (
            <div className={styles.profileView}>
              <GasProfile profile={gasProfile} projectRoot={projectRoot} />
            </div>
          ) : activeView === "coverage" && coverageLcov !== undefined ? (
            <Coverage lcov={coverageLcov} projectRoot={projectRoot} />
          ) : selectedTest ? (
            <TestDetails
              test={selectedTest}
              trace={currentTrace}
              traceError={currentTraceError}
              isTraceLoading={isCurrentTraceLoading}
              projectRoot={projectRoot}
              gasProfile={selectedTestGasProfile}
              gasProfileLoaded={gasProfileLoaded}
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
