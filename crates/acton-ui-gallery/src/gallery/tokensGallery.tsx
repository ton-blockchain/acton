import {cx} from "@acton/ui"
import tokenSource from "@acton/ui/styles/tokens.css?raw"
import type {CSSProperties} from "react"

import styles from "./tokensGallery.module.css"
import type {ComponentGallery} from "./types"

type TokenKind =
  | "color"
  | "border"
  | "shadow"
  | "fontFamily"
  | "fontSize"
  | "fontWeight"
  | "spacing"
  | "radius"
  | "motion"

const tokenNames = [
  ...new Set(
    [...tokenSource.matchAll(/(--acton-[\w-]+)\s*:/g)].flatMap(match =>
      match[1] ? [match[1]] : [],
    ),
  ),
]

const groupDefinitions = [
  {
    id: "surfaces",
    title: "Surfaces",
    matches: (token: string) =>
      token.startsWith("--acton-color-") &&
      !token.includes("popover") &&
      (token.includes("surface") || token.endsWith("-canvas")),
  },
  {
    id: "content",
    title: "Content",
    matches: (token: string) => token.startsWith("--acton-color-text"),
  },
  {
    id: "borders",
    title: "Borders",
    matches: (token: string) => token.startsWith("--acton-color-border"),
  },
  {
    id: "actions",
    title: "Actions",
    matches: (token: string) =>
      token.startsWith("--acton-color-primary") || token.startsWith("--acton-color-accent"),
  },
  {
    id: "feedback",
    title: "Feedback",
    matches: (token: string) =>
      token.startsWith("--acton-color-success") ||
      token.startsWith("--acton-color-danger") ||
      token.startsWith("--acton-color-skeleton"),
  },
  {
    id: "overlays",
    title: "Overlays",
    matches: (token: string) =>
      token.startsWith("--acton-color-focus") ||
      token.startsWith("--acton-color-backdrop") ||
      token.startsWith("--acton-color-popover"),
  },
  {
    id: "typography",
    title: "Typography",
    matches: (token: string) => token.startsWith("--acton-font-"),
  },
  {
    id: "spacing",
    title: "Spacing",
    matches: (token: string) => token.startsWith("--acton-space-"),
  },
  {
    id: "shape",
    title: "Shape",
    matches: (token: string) => token.startsWith("--acton-radius-"),
  },
  {
    id: "motion",
    title: "Motion",
    matches: (token: string) =>
      token.startsWith("--acton-duration-") || token.startsWith("--acton-ease-"),
  },
  {
    id: "elevation",
    title: "Elevation",
    matches: (token: string) => token.startsWith("--acton-shadow-"),
  },
] as const

const tokenGroups = groupDefinitions.flatMap(group => {
  const tokens = tokenNames.filter(group.matches)
  return tokens.length > 0 ? [{...group, tokens}] : []
})

const classifiedTokens = new Set(tokenGroups.flatMap(group => group.tokens))
const unclassifiedTokens = tokenNames.filter(token => !classifiedTokens.has(token))
const allTokenGroups =
  unclassifiedTokens.length > 0
    ? [...tokenGroups, {id: "other", title: "Other", tokens: unclassifiedTokens}]
    : tokenGroups

function TokenGrid({tokens}: Readonly<{tokens: readonly string[]}>) {
  return (
    <div className={styles.grid}>
      {tokens.map(token => (
        <TokenSample key={token} token={token} />
      ))}
    </div>
  )
}

function TokenSample({token}: Readonly<{token: string}>) {
  const kind = getTokenKind(token)
  const value = `var(${token})`
  const previewStyle: CSSProperties =
    kind === "border"
      ? {borderColor: value}
      : kind === "shadow"
        ? {boxShadow: value}
        : kind === "fontFamily"
          ? {fontFamily: value}
          : kind === "fontSize"
            ? {fontSize: value}
            : kind === "fontWeight"
              ? {fontWeight: value}
              : kind === "radius"
                ? {borderRadius: value}
                : {}
  const motionStyle: CSSProperties = token.startsWith("--acton-duration-")
    ? {transitionDuration: value}
    : {transitionTimingFunction: value}

  return (
    <article className={styles.sample}>
      <div
        className={cx(styles.preview, styles[getPreviewClassName(kind)])}
        style={previewStyle}
        aria-hidden="true"
      >
        {kind === "color" ? (
          <span className={styles.colorFill} style={{backgroundColor: value}} />
        ) : kind === "spacing" ? (
          <span className={styles.spacingMark} style={{width: value, height: value}} />
        ) : kind === "motion" ? (
          <span className={styles.motionMark} style={motionStyle}>
            →
          </span>
        ) : kind.startsWith("font") ? (
          <span>Aa</span>
        ) : undefined}
      </div>
      <code className={styles.name}>{token}</code>
    </article>
  )
}

function getTokenKind(token: string): TokenKind {
  if (token.startsWith("--acton-color-border") || token.endsWith("-border")) return "border"
  if (token.startsWith("--acton-color-")) return "color"
  if (token.startsWith("--acton-shadow-")) return "shadow"
  if (token === "--acton-font-sans" || token === "--acton-font-mono") return "fontFamily"
  if (token.startsWith("--acton-font-size-")) return "fontSize"
  if (token.startsWith("--acton-font-weight-")) return "fontWeight"
  if (token.startsWith("--acton-space-")) return "spacing"
  if (token.startsWith("--acton-radius-")) return "radius"
  return "motion"
}

function getPreviewClassName(kind: TokenKind) {
  if (kind.startsWith("font")) return "font"
  return kind
}

export const tokensGallery = {
  kind: "foundation",
  id: "tokens",
  title: "Tokens",
  status: "ready",
  summary: "Generated directly from the CSS custom properties exported by @acton/ui.",
  sections: allTokenGroups.map(group => ({
    id: `tokens-${group.id}`,
    title: group.title,
    content: <TokenGrid tokens={group.tokens} />,
  })),
} satisfies ComponentGallery
