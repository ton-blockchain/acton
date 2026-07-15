import type React from "react"
import {useEffect, useState} from "react"

import {HighlightedCode} from "@acton/ui"

import styles from "./CodeSnippet.module.css"

interface CodeSnippetProps {
  readonly filePath: string
  readonly line: number
  readonly contextLines?: number
  readonly projectRoot?: string
  readonly ideOpener?: React.ReactNode
}

export const CodeSnippet: React.FC<CodeSnippetProps> = ({
  filePath,
  line,
  contextLines = 5,
  projectRoot,
  ideOpener,
}) => {
  const [snippet, setSnippet] = useState<string | undefined>()
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | undefined>()

  const relativePath =
    projectRoot && filePath.startsWith(projectRoot)
      ? filePath.slice(projectRoot.length) || filePath
      : filePath

  useEffect(() => {
    const controller = new AbortController()

    const loadContent = async () => {
      setLoading(true)
      setError(undefined)
      try {
        const result = await fetch(`/api/file?path=${encodeURIComponent(filePath)}`, {
          signal: controller.signal,
        })
        if (!result.ok) throw new Error("Failed to fetch file content")
        const content = await result.text()

        const lines = content.split("\n")
        const start = Math.max(0, line - contextLines - 1)
        const end = Math.min(lines.length, line + contextLines)
        const snippetLines = lines.slice(start, end)
        setSnippet(snippetLines.join("\n"))
        setLoading(false)
      } catch (error: unknown) {
        if (error instanceof Error && error.name === "AbortError") return
        console.error(error)
        setError((error as {message: string}).message)
        setLoading(false)
      }
    }

    void loadContent()
    return () => controller.abort()
  }, [filePath, line, contextLines])

  if (loading) return <div className={styles.loading}>Loading code snippet...</div>
  if (error) return <div className={styles.error}>Error: {error}</div>
  if (snippet === undefined) return

  const startLine = Math.max(1, line - contextLines)
  const snippetLines = snippet.split("\n")

  return (
    <div className={styles.container}>
      <div className={styles.header}>
        <div className={styles.headerLeft}>
          <span className={styles.filePath} title={filePath}>
            {relativePath}
          </span>
          {ideOpener}
        </div>
      </div>
      <div className={styles.codeWrapper}>
        <div className={styles.lineNumbers}>
          {snippetLines.map((_, index) => (
            <div
              key={startLine + index}
              className={`${styles.lineNumber} ${startLine + index === line ? styles.activeLineNumber : ""}`}
            >
              {startLine + index}
            </div>
          ))}
        </div>
        <HighlightedCode className={styles.shikiWrapper} value={snippet} language="tolk" />
        {startLine + snippetLines.findIndex((_, index) => startLine + index === line) === line && (
          <div
            className={styles.activeLineOverlay}
            style={{
              top: `${(line - startLine) * 1.5 + 0.5}rem`,
              height: "1.5rem",
            }}
          />
        )}
      </div>
    </div>
  )
}
