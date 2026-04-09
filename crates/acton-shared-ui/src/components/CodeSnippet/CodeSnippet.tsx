import type React from "react"
import {useEffect, useState} from "react"

import styles from "./CodeSnippet.module.css"
import {highlightTolkToHtml} from "./tolk-highlighter"

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
  const [highlightedHtml, setHighlightedHtml] = useState<string | undefined>()
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | undefined>()

  const relativePath =
    projectRoot && filePath.startsWith(projectRoot)
      ? filePath.slice(projectRoot.length) || filePath
      : filePath

  useEffect(() => {
    const loadContent = async () => {
      setLoading(true)
      try {
        const result = await fetch(`/api/file?path=${encodeURIComponent(filePath)}`)
        if (!result.ok) throw new Error("Failed to fetch file content")
        const content = await result.text()

        const lines = content.split("\n")
        const start = Math.max(0, line - contextLines - 1)
        const end = Math.min(lines.length, line + contextLines)
        const snippetLines = lines.slice(start, end)
        const snippetText = snippetLines.join("\n")

        setSnippet(snippetText)

        const isDark = document.documentElement.classList.contains("dark-theme")
        const html = await highlightTolkToHtml(snippetText, isDark)

        setHighlightedHtml(html)
        setLoading(false)
      } catch (error: unknown) {
        console.error(error)
        setError((error as {message: string}).message)
        setLoading(false)
      }
    }

    void loadContent()

    // Listen for theme changes
    const observer = new MutationObserver(mutations => {
      for (const mutation of mutations) {
        if (mutation.type === "attributes" && mutation.attributeName === "class") {
          void loadContent()
        }
      }
    })

    observer.observe(document.documentElement, {attributes: true})
    return () => observer.disconnect()
  }, [filePath, line, contextLines])

  if (loading) return <div className={styles.loading}>Loading code snippet...</div>
  if (error) return <div className={styles.error}>Error: {error}</div>
  if (!snippet || !highlightedHtml) return

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
        <div className={styles.shikiWrapper} dangerouslySetInnerHTML={{__html: highlightedHtml}} />
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
