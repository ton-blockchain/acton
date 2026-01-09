import type React from "react"
import { useCallback, useEffect, useRef, useState } from "react"
import { FiChevronRight } from "react-icons/fi"
import { Sidebar } from "./components/Sidebar/Sidebar"
import { TestDetails } from "./components/TestDetails/TestDetails"
import type { TestReport, Trace } from "./types"

export const App: React.FC = () => {
  const [reports, setReports] = useState<TestReport[]>([])
  const [selectedTest, setSelectedTest] = useState<TestReport | null>(null)
  const [currentTrace, setCurrentTrace] = useState<Trace | null>(null)
  const [theme, setTheme] = useState(() => {
    return (
      localStorage.getItem("theme") ||
      (window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light")
    )
  })
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    document.documentElement.classList.toggle("dark-theme", theme === "dark")
    localStorage.setItem("theme", theme)
  }, [theme])

  const toggleTheme = useCallback(() => {
    setTheme((prev) => (prev === "light" ? "dark" : "light"))
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
      fetch(`/api/trace/${test.trace_path}`)
        .then((res) => res.json())
        .then((data) => {
          setCurrentTrace(data)
        })
        .catch((err) => {
          console.error("Failed to fetch trace", err)
          setCurrentTrace(null)
        })
    } else {
      setCurrentTrace(null)
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
    setIsSidebarCollapsed((prev) => {
      const newState = !prev
      localStorage.setItem("isSidebarCollapsed", newState.toString())
      return newState
    })
  }, [])

  useEffect(() => {
    fetch("/api/reports")
      .then((res) => res.json())
      .then((data: TestReport[]) => {
        setReports(data)
        if (data.length > 0 && !selectedTest) {
          const savedTestId = localStorage.getItem("selectedTest")
          let testToSelect = data[0]

          if (savedTestId) {
            const found = data.find((t) => `${t.suite_name}::${t.name}` === savedTestId)
            if (found) {
              testToSelect = found
            }
          }
          handleSelectTest(testToSelect)
        }
        setLoading(false)
      })
      .catch((err) => {
        console.error("Failed to fetch reports", err)
        setLoading(false)
      })
  }, [handleSelectTest, selectedTest])

  if (loading && reports.length === 0) {
    return (
      <div
        style={{ display: "flex", justifyContent: "center", alignItems: "center", height: "100vh" }}
      >
        Loading...
      </div>
    )
  }

  return (
    <div style={{ display: "flex", height: "100vh", overflow: "hidden", position: "relative" }}>
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
          style={{
            position: "absolute",
            left: "12px",
            top: "12px",
            width: "32px",
            height: "32px",
            borderRadius: "6px",
            backgroundColor: "var(--card-bg)",
            border: "1px solid var(--border-color)",
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            cursor: "pointer",
            zIndex: 100,
            color: "var(--text-secondary)",
            boxShadow: "var(--shadow)",
          }}
          title="Expand sidebar"
        >
          <FiChevronRight size={20} />
        </button>
      )}

      <div
        onMouseDown={startResizing}
        onMouseEnter={() => setIsHoveredResizer(true)}
        onMouseLeave={() => setIsHoveredResizer(false)}
        role="separator"
        tabIndex={0}
        aria-valuenow={isSidebarCollapsed ? 0 : sidebarWidth}
        aria-valuemin={200}
        aria-valuemax={800}
        aria-label="Resize sidebar"
        style={{
          width: "4px",
          cursor: isSidebarCollapsed ? "default" : "col-resize",
          backgroundColor:
            isHoveredResizer && !isSidebarCollapsed ? "var(--color-todo)" : "transparent",
          transition: "background-color 0.2s",
          borderLeft: "1px solid var(--border-color)",
          zIndex: 10,
          flexShrink: 0,
          outline: "none",
          position: "relative",
        }}
      />

      <div style={{ flex: 1, position: "relative", minWidth: 0 }}>
        {selectedTest ? (
          <TestDetails test={selectedTest} trace={currentTrace} />
        ) : (
          <div
            style={{
              display: "flex",
              justifyContent: "center",
              alignItems: "center",
              height: "100%",
            }}
          >
            Select a test to see details
          </div>
        )}
      </div>
    </div>
  )
}
