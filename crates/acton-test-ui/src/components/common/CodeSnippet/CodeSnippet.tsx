import type React from "react"
import { useEffect, useState } from "react"
import { createHighlighterCore } from "shiki/core"
import { createOnigurumaEngine } from "shiki/engine/oniguruma"
import vitesseDark from "shiki/themes/vitesse-dark.mjs"
import vitesseLight from "shiki/themes/vitesse-light.mjs"
import styles from "./CodeSnippet.module.css"
import { tolkGrammar } from "./tolk-grammar"

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
  const [snippet, setSnippet] = useState<string | null>(null)
  const [highlightedHtml, setHighlightedHtml] = useState<string | null>(null)
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)

  const relativePath =
    projectRoot && filePath.startsWith(projectRoot)
      ? filePath.substring(projectRoot.length) || filePath
      : filePath

  useEffect(() => {
    const loadContent = async () => {
      setLoading(true)
      try {
        const res = await fetch(`/api/file?path=${encodeURIComponent(filePath)}`)
        if (!res.ok) throw new Error("Failed to fetch file content")
        const content = await res.text()

        const lines = content.split("\n")
        const start = Math.max(0, line - contextLines - 1)
        const end = Math.min(lines.length, line + contextLines)
        const snippetLines = lines.slice(start, end)
        const snippetText = snippetLines.join("\n")

        setSnippet(snippetText)

        const highlighter = await createHighlighterCore({
          themes: [vitesseLight, vitesseDark],
          langs: [tolkGrammar],
          engine: createOnigurumaEngine(() => import("shiki/wasm")),
        })

        const isDark = document.documentElement.classList.contains("dark-theme")
        // noinspection TypeScriptValidateTypes
        const html = highlighter.codeToHtml(snippetText, {
          lang: "tolk",
          theme: isDark ? "vitesse-dark" : "vitesse-light",
        })

        setHighlightedHtml(html)
        setLoading(false)
      } catch (err: unknown) {
        console.error(err)
        setError(err.message)
        setLoading(false)
      }
    }

    loadContent()

    // Listen for theme changes
    const observer = new MutationObserver((mutations) => {
      for (const mutation of mutations) {
        if (mutation.type === "attributes" && mutation.attributeName === "class") {
          loadContent()
        }
      }
    })

    observer.observe(document.documentElement, { attributes: true })
    return () => observer.disconnect()
  }, [filePath, line, contextLines])

  if (loading) return <div className={styles.loading}>Loading code snippet...</div>
  if (error) return <div className={styles.error}>Error: {error}</div>
  if (!snippet || !highlightedHtml) return null

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
          {snippetLines.map((_, i) => (
            <div
              key={startLine + i}
              className={`${styles.lineNumber} ${startLine + i === line ? styles.activeLineNumber : ""}`}
            >
              {startLine + i}
            </div>
          ))}
        </div>
        <div
          className={styles.shikiWrapper}
          dangerouslySetInnerHTML={{ __html: highlightedHtml }}
        />
        {startLine + snippetLines.findIndex((_, i) => startLine + i === line) === line && (
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
