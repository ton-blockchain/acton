import type React from "react"
import {useEffect, useMemo, useRef, useState} from "react"
import {FiSearch} from "react-icons/fi"

import type {HighlightedToken} from "@acton/shared-ui"
import {highlightTolkToTokens} from "@acton/shared-ui"

import {parseLcov, type CoverageFile} from "../../utils/lcov"

import styles from "./Coverage.module.css"

interface CoverageProps {
  readonly lcov: string
  readonly projectRoot?: string
}

const getRelativePath = (filePath: string, projectRoot?: string) => {
  if (projectRoot && filePath.startsWith(projectRoot)) {
    const relativePath = filePath.slice(projectRoot.length)
    return relativePath || filePath
  }

  const pathSegments = filePath.split("/")
  if (pathSegments.length > 4) {
    return `.../${pathSegments.slice(-4).join("/")}`
  }

  return filePath
}

const formatPercentage = (value: number | undefined) => {
  if (value === undefined) {
    return "n/a"
  }

  return `${value.toFixed(1)}%`
}

const formatBranchCoverage = (file: CoverageFile) => {
  if (file.branchesFound === 0) {
    return "No branches"
  }

  return `${file.branchesHit}/${file.branchesFound} branches`
}

const getScoreTone = (score: number) => {
  if (score >= 85) {
    return styles.scoreOk
  }

  if (score >= 60) {
    return styles.scoreWarn
  }

  return styles.scoreCritical
}

const ITALIC_FONT_STYLE = 1
const BOLD_FONT_STYLE = 2
const UNDERLINE_FONT_STYLE = 4
const STRIKETHROUGH_FONT_STYLE = 8

const tokenStyle = (token: HighlightedToken): React.CSSProperties | undefined => {
  const style: React.CSSProperties = {}

  if (token.htmlStyle) {
    return token.htmlStyle as React.CSSProperties
  }

  if (token.color) {
    style.color = token.color
  }

  const fontStyle = token.fontStyle ?? 0
  if ((fontStyle & ITALIC_FONT_STYLE) !== 0) {
    style.fontStyle = "italic"
  }
  if ((fontStyle & BOLD_FONT_STYLE) !== 0) {
    style.fontWeight = "bold"
  }

  const textDecorations: string[] = []
  if ((fontStyle & UNDERLINE_FONT_STYLE) !== 0) {
    textDecorations.push("underline")
  }
  if ((fontStyle & STRIKETHROUGH_FONT_STYLE) !== 0) {
    textDecorations.push("line-through")
  }
  if (textDecorations.length > 0) {
    style.textDecoration = textDecorations.join(" ")
  }

  return Object.keys(style).length > 0 ? style : undefined
}

export const Coverage: React.FC<CoverageProps> = ({lcov, projectRoot}) => {
  const coverage = useMemo(() => parseLcov(lcov), [lcov])
  const [searchQuery, setSearchQuery] = useState("")
  const [selectedFilePath, setSelectedFilePath] = useState<string | undefined>(() => {
    return coverage.files[0]?.filePath
  })
  const [sourceContent, setSourceContent] = useState("")
  const [sourceError, setSourceError] = useState<string | undefined>()
  const [isLoadingSource, setIsLoadingSource] = useState(false)
  const [highlightedLines, setHighlightedLines] = useState<
    readonly (readonly HighlightedToken[])[] | undefined
  >()
  const codePaneRef = useRef<HTMLDivElement | null>(null)

  const filteredFiles = useMemo(() => {
    const normalizedQuery = searchQuery.trim().toLowerCase()

    return coverage.files.filter(file => {
      const matchesQuery =
        normalizedQuery.length === 0 ||
        file.filePath.toLowerCase().includes(normalizedQuery) ||
        getRelativePath(file.filePath, projectRoot).toLowerCase().includes(normalizedQuery)

      return matchesQuery
    })
  }, [coverage.files, projectRoot, searchQuery])

  useEffect(() => {
    if (filteredFiles.some(file => file.filePath === selectedFilePath)) {
      return
    }

    setSelectedFilePath(filteredFiles[0]?.filePath ?? coverage.files[0]?.filePath)
  }, [coverage.files, filteredFiles, selectedFilePath])

  const selectedFile = useMemo(() => {
    if (selectedFilePath === undefined) {
      return filteredFiles[0] ?? coverage.files[0]
    }

    return coverage.files.find(file => file.filePath === selectedFilePath) ?? filteredFiles[0]
  }, [coverage.files, filteredFiles, selectedFilePath])

  useEffect(() => {
    if (selectedFile === undefined) {
      setSourceContent("")
      setSourceError(undefined)
      setIsLoadingSource(false)
      setHighlightedLines(undefined)
      return
    }

    const controller = new AbortController()

    setIsLoadingSource(true)
    setSourceError(undefined)
    setSourceContent("")
    setHighlightedLines(undefined)

    void fetch(`/api/file?path=${encodeURIComponent(selectedFile.filePath)}`, {
      signal: controller.signal,
    })
      .then(async response => {
        if (!response.ok) {
          throw new Error(`Failed to fetch source file: ${response.status}`)
        }

        return response.text()
      })
      .then(content => {
        setSourceContent(content)
        setIsLoadingSource(false)
      })
      .catch(error => {
        if (error instanceof Error && error.name === "AbortError") {
          return
        }

        console.error("Failed to fetch coverage source file", error)
        setSourceError(error instanceof Error ? error.message : "Unknown error")
        setIsLoadingSource(false)
      })

    return () => controller.abort()
  }, [selectedFile])

  useEffect(() => {
    if (!sourceContent) {
      setHighlightedLines(undefined)
      return
    }

    let isDisposed = false

    const renderHighlightedLines = async () => {
      try {
        const isDark = document.documentElement.classList.contains("dark-theme")
        const tokens = await highlightTolkToTokens(sourceContent, isDark)
        if (!isDisposed) {
          setHighlightedLines(tokens)
        }
      } catch (error) {
        console.error("Failed to highlight coverage source file", error)
        if (!isDisposed) {
          setHighlightedLines(undefined)
        }
      }
    }

    void renderHighlightedLines()

    const observer = new MutationObserver(mutations => {
      for (const mutation of mutations) {
        if (mutation.type === "attributes" && mutation.attributeName === "class") {
          void renderHighlightedLines()
        }
      }
    })

    observer.observe(document.documentElement, {attributes: true})

    return () => {
      isDisposed = true
      observer.disconnect()
    }
  }, [sourceContent])

  const focusLine = selectedFile?.firstUncoveredLine ?? selectedFile?.firstPartialLine

  useEffect(() => {
    if (focusLine === undefined || !sourceContent) {
      return
    }

    const targetLine = codePaneRef.current?.querySelector<HTMLElement>(
      `[data-line-number="${focusLine}"]`,
    )
    targetLine?.scrollIntoView({block: "center"})
  }, [focusLine, sourceContent])

  const sourceLines = useMemo(() => {
    if (!sourceContent) {
      return []
    }

    return sourceContent.split("\n")
  }, [sourceContent])

  if (coverage.files.length === 0) {
    return <div className={styles.emptyState}>No coverage records found in the LCOV report.</div>
  }

  return (
    <div className={styles.coverage}>
      <div className={styles.summaryGrid}>
        <div className={styles.summaryCard}>
          <div className={styles.summaryLabel}>Overall Score</div>
          <div className={`${styles.summaryValue} ${getScoreTone(coverage.combinedScore)}`}>
            {formatPercentage(coverage.combinedScore)}
          </div>
          <div className={styles.summaryMeta}>Weighted across lines and branches</div>
        </div>
        <div className={styles.summaryCard}>
          <div className={styles.summaryLabel}>Line Coverage</div>
          <div className={styles.summaryValue}>{formatPercentage(coverage.linePercentage)}</div>
          <div className={styles.summaryMeta}>
            {coverage.totalLinesHit}/{coverage.totalLinesFound} executable lines
          </div>
        </div>
        <div className={styles.summaryCard}>
          <div className={styles.summaryLabel}>Branch Coverage</div>
          <div className={styles.summaryValue}>{formatPercentage(coverage.branchPercentage)}</div>
          <div className={styles.summaryMeta}>
            {coverage.totalBranchesFound > 0
              ? `${coverage.totalBranchesHit}/${coverage.totalBranchesFound} branches`
              : "No branches recorded"}
          </div>
        </div>
        <div className={styles.summaryCard}>
          <div className={styles.summaryLabel}>Files</div>
          <div className={styles.summaryValue}>{coverage.totalFiles}</div>
          <div className={styles.summaryMeta}>Sorted by lowest score first</div>
        </div>
      </div>

      <div className={styles.workspace}>
        <aside className={styles.sidebar}>
          <div className={styles.sidebarHeader}>
            <div className={styles.sidebarTitle}>Coverage Files</div>
            <div className={styles.sidebarMeta}>Sorted by score</div>
          </div>

          <div className={styles.searchContainer}>
            <FiSearch className={styles.searchIcon} />
            <input
              type="search"
              value={searchQuery}
              onChange={event => setSearchQuery(event.target.value)}
              placeholder="Filter files..."
              className={styles.searchInput}
            />
          </div>

          <div className={styles.fileList}>
            {filteredFiles.map(file => {
              const isSelected = file.filePath === selectedFile?.filePath
              const relativePath = getRelativePath(file.filePath, projectRoot)

              return (
                <button
                  key={file.filePath}
                  type="button"
                  className={`${styles.fileButton} ${isSelected ? styles.fileButtonSelected : ""}`}
                  onClick={() => setSelectedFilePath(file.filePath)}
                >
                  <div className={styles.fileRow}>
                    <span className={styles.filePath} title={file.filePath}>
                      {relativePath}
                    </span>
                    <span
                      className={`${styles.filePercentage} ${getScoreTone(file.combinedScore)}`}
                    >
                      {formatPercentage(file.combinedScore)}
                    </span>
                  </div>
                  <div className={styles.fileMeta}>
                    <span>
                      {file.linesHit}/{file.linesFound} lines
                    </span>
                    <span>{formatBranchCoverage(file)}</span>
                  </div>
                  <div className={styles.progressTrack}>
                    <div
                      className={`${styles.progressFill} ${getScoreTone(file.combinedScore)}`}
                      style={{width: `${Math.max(2, file.combinedScore)}%`}}
                    />
                  </div>
                </button>
              )
            })}

            {filteredFiles.length === 0 && (
              <div className={styles.emptyList}>No files matched the current filter.</div>
            )}
          </div>
        </aside>

        <section className={styles.viewer}>
          {selectedFile === undefined ? (
            <div className={styles.emptyState}>Select a file to inspect its coverage.</div>
          ) : (
            <>
              <div className={styles.viewerHeader}>
                <div>
                  <div className={styles.viewerPath} title={selectedFile.filePath}>
                    {getRelativePath(selectedFile.filePath, projectRoot)}
                  </div>
                  <div className={styles.viewerMeta}>
                    <span
                      className={`${styles.viewerScore} ${getScoreTone(selectedFile.combinedScore)}`}
                    >
                      Score {formatPercentage(selectedFile.combinedScore)}
                    </span>
                    <span>
                      {selectedFile.linesHit}/{selectedFile.linesFound} executable lines
                    </span>
                    <span>{formatBranchCoverage(selectedFile)}</span>
                    {focusLine !== undefined && <span>First gap at line {focusLine}</span>}
                  </div>
                </div>
              </div>

              <div className={styles.legend}>
                <span className={`${styles.legendItem} ${styles.legendCovered}`}>Covered</span>
                <span className={`${styles.legendItem} ${styles.legendPartial}`}>
                  Partial branch
                </span>
                <span className={`${styles.legendItem} ${styles.legendUncovered}`}>Uncovered</span>
              </div>

              {isLoadingSource && <div className={styles.viewerState}>Loading source file...</div>}
              {!isLoadingSource && sourceError !== undefined && (
                <div className={styles.viewerState}>Unable to load source: {sourceError}</div>
              )}
              {!isLoadingSource && sourceError === undefined && (
                <div className={styles.codePane} ref={codePaneRef}>
                  {sourceLines.map((sourceLine, index) => {
                    const lineNumber = index + 1
                    const lineCoverage = selectedFile.lines.get(lineNumber)
                    const lineTokens = highlightedLines?.[index]
                    const lineClassName =
                      lineCoverage?.status === "covered"
                        ? styles.codeLineCovered
                        : lineCoverage?.status === "partial"
                          ? styles.codeLinePartial
                          : lineCoverage?.status === "uncovered"
                            ? styles.codeLineUncovered
                            : ""

                    return (
                      <div
                        key={lineNumber}
                        data-line-number={lineNumber}
                        className={`${styles.codeLine} ${lineClassName}`}
                      >
                        <div className={styles.lineNumber}>{lineNumber}</div>
                        <div className={styles.hitCount}>
                          {lineCoverage ? `${lineCoverage.hits}x` : ""}
                        </div>
                        <div className={styles.codeText}>
                          {lineTokens && lineTokens.length > 0
                            ? lineTokens.map((token, tokenIndex) => (
                                <span
                                  key={`${lineNumber}-${tokenIndex}`}
                                  style={tokenStyle(token)}
                                  className={styles.codeToken}
                                >
                                  {token.content}
                                </span>
                              ))
                            : sourceLine || " "}
                        </div>
                        <div className={styles.branchInfo}>
                          {lineCoverage && lineCoverage.branchesFound > 0
                            ? `${lineCoverage.branchesHit}/${lineCoverage.branchesFound}`
                            : ""}
                        </div>
                      </div>
                    )
                  })}
                </div>
              )}
            </>
          )}
        </section>
      </div>
    </div>
  )
}
