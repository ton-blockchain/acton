import * as React from "react"
import {useCallback, useEffect, useRef, useState} from "react"
import {FiChevronRight} from "react-icons/fi"

import type {TestReport, Trace} from "@acton/shared-ui"

import styles from "./App.module.css"
import {Coverage} from "./components/Coverage/Coverage"
import {Sidebar} from "./components/Sidebar/Sidebar"
import {TestDetails} from "./components/TestDetails/TestDetails"

export const App: React.FC = () => {
  const [reports, setReports] = useState<TestReport[]>([])
  const [selectedTest, setSelectedTest] = useState<TestReport | undefined>()
  const [currentTrace, setCurrentTrace] = useState<Trace | undefined>()
  const [projectRoot, setProjectRoot] = useState<string>("")
  const [theme, setTheme] = useState(() => {
    return (
      localStorage.getItem("theme") ||
      (globalThis.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light")
    )
  })
  const [loading, setLoading] = useState(true)
  const [coverageLcov, setCoverageLcov] = useState<string | undefined>()
  const [coverageLoaded, setCoverageLoaded] = useState(false)
  const [activeView, setActiveView] = useState<"tests" | "coverage">(() => {
    const saved = localStorage.getItem("activeMainView")
    return saved === "coverage" ? "coverage" : "tests"
  })

  useEffect(() => {
    document.documentElement.classList.toggle("dark-theme", theme === "dark")
    localStorage.setItem("theme", theme)
  }, [theme])

  const toggleTheme = useCallback(() => {
    setTheme(prev => (prev === "light" ? "dark" : "light"))
  }, [])
  const handleActiveViewChange = useCallback((view: "tests" | "coverage") => {
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
  const [isHoveredResizer, setIsHoveredResizer] = useState(false)
  const isResizing = useRef(false)
  const lastWidth = useRef(sidebarWidth)

  const handleSelectTest = useCallback((test: TestReport) => {
    setSelectedTest(test)
    localStorage.setItem("selectedTest", `${test.suite_name}::${test.name}`)
    if (test.trace_path) {
      void fetch(`/api/trace/${test.trace_path}`)
        .then(async res => (await res.json()) as Trace)
        .then(data => {
          setCurrentTrace(data)
        })
        .catch(error => {
          console.error("Failed to fetch trace", error)
          setCurrentTrace(undefined)
        })
    } else {
      setCurrentTrace(undefined)
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
    document.removeEventListener("mousemove", handleMouseMove)
    document.removeEventListener("mouseup", stopResizing)
    document.body.style.cursor = ""
    document.body.style.userSelect = ""
  }, [handleMouseMove])

  const startResizing = useCallback(() => {
    if (isSidebarCollapsed) return
    isResizing.current = true
    document.addEventListener("mousemove", handleMouseMove)
    document.addEventListener("mouseup", stopResizing)
    document.body.style.cursor = "col-resize"
    document.body.style.userSelect = "none"
  }, [handleMouseMove, stopResizing, isSidebarCollapsed])

  const toggleSidebar = useCallback(() => {
    setIsSidebarCollapsed(prev => {
      const newState = !prev
      localStorage.setItem("isSidebarCollapsed", newState.toString())
      return newState
    })
  }, [])

  useEffect(() => {
    const coverageController = new AbortController()
    const reportsController = new AbortController()
    const configController = new AbortController()

    void fetch("/api/config", {signal: configController.signal})
      .then(async res => (await res.json()) as {project_root: string})
      .then(data => {
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
        if (response.status === 404) {
          setCoverageLcov(undefined)
          setCoverageLoaded(true)
          return
        }

        if (!response.ok) {
          throw new Error(`Failed to fetch coverage report: ${response.status}`)
        }

        const lcov = await response.text()
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

    return () => {
      coverageController.abort()
      reportsController.abort()
      configController.abort()
    }
  }, [])

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
  }, [activeView, coverageLcov, coverageLoaded, handleActiveViewChange])

  if (loading && reports.length === 0) {
    return <div className={styles.loadingContainer}>Loading...</div>
  }

  return (
    <div className={styles.app}>
      {!isSidebarCollapsed && (
        <Sidebar
          reports={reports}
          selectedTest={selectedTest}
          onSelectTest={handleSelectTest}
          width={sidebarWidth}
          onCollapse={toggleSidebar}
          theme={theme}
          onToggleTheme={toggleTheme}
        />
      )}

      {isSidebarCollapsed && (
        <button
          type="button"
          onClick={toggleSidebar}
          className={styles.expandButton}
          title="Expand sidebar"
        >
          <FiChevronRight size={20} />
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
        {coverageLcov !== undefined && (
          <div className={styles.viewTabs}>
            <button
              type="button"
              className={`${styles.viewTab} ${activeView === "tests" ? styles.viewTabActive : ""}`}
              onClick={() => handleActiveViewChange("tests")}
            >
              Tests
            </button>
            <button
              type="button"
              className={`${styles.viewTab} ${activeView === "coverage" ? styles.viewTabActive : ""}`}
              onClick={() => handleActiveViewChange("coverage")}
            >
              Coverage
            </button>
          </div>
        )}

        <div className={styles.mainPanel}>
          {activeView === "coverage" && coverageLcov !== undefined ? (
            <Coverage lcov={coverageLcov} projectRoot={projectRoot} />
          ) : selectedTest ? (
            <TestDetails test={selectedTest} trace={currentTrace} projectRoot={projectRoot} />
          ) : (
            <div className={styles.noSelection}>Select a test to see details</div>
          )}
        </div>
      </div>
    </div>
  )
}
