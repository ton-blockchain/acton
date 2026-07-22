import type {ReactNode} from "react"
import {useMemo} from "react"

import {HighlightedCode} from "@acton/ui"

import {useFileContent} from "../../hooks/useFileContent"

import styles from "./CodeSnippet.module.css"

interface CodeSnippetProps {
  readonly filePath: string
  readonly line: number
  readonly contextLines?: number
  readonly projectRoot?: string
  readonly ideOpener?: ReactNode
}

export function CodeSnippet({
  filePath,
  line,
  contextLines = 5,
  projectRoot,
  ideOpener,
}: CodeSnippetProps) {
  const {content, loading, error} = useFileContent(filePath)
  const snippet = useMemo(() => {
    if (content === undefined) return

    const lines = content.split("\n")
    const start = Math.max(0, line - contextLines - 1)
    const end = Math.min(lines.length, line + contextLines)
    return lines.slice(start, end).join("\n")
  }, [content, contextLines, line])

  const relativePath =
    projectRoot && filePath.startsWith(projectRoot)
      ? filePath.slice(projectRoot.length) || filePath
      : filePath

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
