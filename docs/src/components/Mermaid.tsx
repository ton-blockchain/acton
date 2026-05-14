"use client"

import mermaid from "mermaid"
import {useEffect, useId, useState} from "react"

type MermaidProps = {
  chart: string
}

export function Mermaid({chart}: MermaidProps) {
  const reactId = useId()
  const elementId = `mermaid-${reactId.replace(/:/g, "")}`
  const [svg, setSvg] = useState<string>("")
  const [themeMode, setThemeMode] = useState<"light" | "dark">("light")

  useEffect(() => {
    const root = document.documentElement

    const updateThemeMode = () => {
      setThemeMode(root.classList.contains("dark") ? "dark" : "light")
    }

    updateThemeMode()

    const observer = new MutationObserver(updateThemeMode)
    observer.observe(root, {attributes: true, attributeFilter: ["class"]})

    return () => {
      observer.disconnect()
    }
  }, [])

  useEffect(() => {
    const styles = getComputedStyle(document.documentElement)
    const foreground = styles.getPropertyValue("--color-fd-foreground").trim()
    const background = styles.getPropertyValue("--color-fd-background").trim()
    const primary = styles.getPropertyValue("--color-fd-primary").trim()

    const palette =
      themeMode === "dark"
        ? {
            surface: "#262626",
            surfaceAlt: "#2d2d2d",
            labelSurface: "#262626",
            border: "rgba(255, 255, 255, 0.22)",
            line: "rgba(255, 255, 255, 0.72)",
            text: "#f5f5f5",
          }
        : {
            surface: "#ffffff",
            surfaceAlt: "#fafafa",
            labelSurface: "#ffffff",
            border: "rgba(0, 0, 0, 0.18)",
            line: "rgba(0, 0, 0, 0.58)",
            text: foreground,
          }

    mermaid.initialize({
      startOnLoad: false,
      securityLevel: "loose",
      theme: "base",
      fontFamily: "var(--font-geist-sans), ui-sans-serif, system-ui, sans-serif",
      themeVariables: {
        darkMode: themeMode === "dark",
        background,
        mainBkg: palette.surface,
        secondBkg: palette.surfaceAlt,
        tertiaryColor: palette.surfaceAlt,
        primaryColor: palette.surface,
        secondaryColor: palette.surface,
        primaryBorderColor: palette.border,
        secondaryBorderColor: palette.border,
        tertiaryBorderColor: palette.border,
        primaryTextColor: palette.text,
        secondaryTextColor: palette.text,
        tertiaryTextColor: palette.text,
        textColor: palette.text,
        nodeTextColor: palette.text,
        lineColor: palette.line,
        edgeLabelBackground: palette.labelSurface,
        clusterBkg: palette.surface,
        clusterBorder: palette.border,
        defaultLinkColor: palette.line,
        titleColor: palette.text,
        actorTextColor: palette.text,
        labelBoxBkgColor: palette.labelSurface,
        labelBoxBorderColor: palette.border,
        signalColor: palette.text,
        signalTextColor: palette.text,
        cScale0: palette.surface,
        cScale1: palette.surfaceAlt,
        cScale2: palette.surface,
        cScale3: palette.surfaceAlt,
        cScale4: palette.surface,
        cScale5: palette.surfaceAlt,
        cScale6: palette.surface,
        cScale7: palette.surfaceAlt,
        pie1: primary,
        pie2: palette.line,
        pie3: palette.border,
      },
      flowchart: {
        htmlLabels: false,
        curve: "basis",
      },
    })

    let cancelled = false

    void mermaid
      .render(elementId, chart)
      .then(result => {
        if (!cancelled) {
          setSvg(result.svg)
        }
      })
      .catch(error => {
        if (!cancelled) {
          setSvg(`<pre>${String(error)}</pre>`)
        }
      })

    return () => {
      cancelled = true
    }
  }, [chart, elementId, themeMode])

  return (
    <div
      data-theme={themeMode}
      className="mermaid-diagram my-6 overflow-x-auto rounded-xl border bg-fd-card px-4 py-4 text-sm shadow-sm [&_svg]:h-auto [&_svg]:max-w-full"
      dangerouslySetInnerHTML={{__html: svg}}
    />
  )
}
