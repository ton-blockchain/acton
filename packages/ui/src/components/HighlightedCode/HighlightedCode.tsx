import {useEffect, useState, type CSSProperties} from "react"

import {cx} from "../../lib/cx"
import {useTheme} from "../Theme/ThemeProvider"
import styles from "./HighlightedCode.module.css"
import type {HighlightedCodeLanguage} from "./types"

export interface HighlightedCodeProps {
  readonly ariaLabel?: string
  readonly className?: string
  /** Omit for consistently styled preformatted text without syntax highlighting. */
  readonly language?: HighlightedCodeLanguage
  readonly maxHeight?: CSSProperties["maxHeight"]
  readonly minHeight?: CSSProperties["minHeight"]
  readonly style?: CSSProperties
  readonly value: string
  readonly wrap?: boolean
}

interface HighlightResult {
  readonly html?: string
  readonly key: string
}

export function HighlightedCode({
  ariaLabel,
  className,
  language,
  maxHeight,
  minHeight,
  style,
  value,
  wrap = false,
}: HighlightedCodeProps) {
  const {theme} = useTheme()
  const [highlighted, setHighlighted] = useState<HighlightResult>({key: ""})
  const highlightKey = `${theme}:${language ?? "plain"}:${value}`

  useEffect(() => {
    if (!language) return

    let isActive = true

    void import("./highlightCodeToHtml")
      .then(({highlightCodeToHtml}) => highlightCodeToHtml(value, language, theme))
      .then(html => {
        if (isActive) setHighlighted({html, key: highlightKey})
      })
      .catch(error => {
        console.error(`Failed to highlight ${language} code`, error)
        if (isActive) setHighlighted({key: highlightKey})
      })

    return () => {
      isActive = false
    }
  }, [highlightKey, language, theme, value])

  const resolvedStyle = {
    ...style,
    ...(maxHeight === undefined
      ? {}
      : {"--acton-highlighted-code-max-height": toCssSize(maxHeight)}),
    ...(minHeight === undefined
      ? {}
      : {"--acton-highlighted-code-min-height": toCssSize(minHeight)}),
  } as CSSProperties
  const html = highlighted.key === highlightKey ? highlighted.html : undefined

  return (
    <div
      className={cx(styles.root, className)}
      data-wrap={wrap ? "true" : undefined}
      style={resolvedStyle}
      aria-label={ariaLabel}
    >
      {html ? (
        <div className={styles.highlighted} dangerouslySetInnerHTML={{__html: html}} />
      ) : (
        <pre className={styles.fallback}>
          <code>{value}</code>
        </pre>
      )}
    </div>
  )
}

function toCssSize(value: CSSProperties["maxHeight"] | CSSProperties["minHeight"]) {
  return typeof value === "number" ? `${value}px` : value
}
