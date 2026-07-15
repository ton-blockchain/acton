import {useEffect, useRef, useState, type CSSProperties} from "react"

import {cx} from "../../lib/cx"
import styles from "./HighlightedCode.module.css"
import type {HighlightedCodeLanguage, HighlightedCodeTheme} from "./types"

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
  const rootRef = useRef<HTMLDivElement>(null)
  const [theme, setTheme] = useState<HighlightedCodeTheme>(() => detectTheme())
  const [highlighted, setHighlighted] = useState<HighlightResult>({key: ""})
  const highlightKey = `${theme}:${language ?? "plain"}:${value}`

  // react-doctor-disable-next-line react-doctor/effect-needs-cleanup -- cleanup disconnects every observer created by this effect
  useEffect(() => {
    const updateTheme = () => setTheme(detectTheme(rootRef.current))
    updateTheme()

    const themeElements = new Set(
      [document.documentElement, rootRef.current?.closest("[data-theme], .dark-theme")].filter(
        (element): element is Element => element !== null && element !== undefined,
      ),
    )
    const observers = [...themeElements].map(element => {
      const observer = new MutationObserver(updateTheme)
      observer.observe(element, {
        attributeFilter: ["class", "data-theme"],
        attributes: true,
      })
      return observer
    })

    return () => {
      for (const observer of observers) observer.disconnect()
    }
  }, [])

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
      ref={rootRef}
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

function detectTheme(element?: Element | null): HighlightedCodeTheme {
  if (typeof document === "undefined") return "light"

  const themeRoot = element?.closest("[data-theme], .dark-theme")
  const isDark =
    themeRoot?.classList.contains("dark-theme") ||
    themeRoot?.getAttribute("data-theme") === "dark" ||
    document.documentElement.classList.contains("dark-theme") ||
    document.documentElement.getAttribute("data-theme") === "dark"

  return isDark ? "dark" : "light"
}

function toCssSize(value: CSSProperties["maxHeight"] | CSSProperties["minHeight"]) {
  return typeof value === "number" ? `${value}px` : value
}
