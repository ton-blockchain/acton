import {Children, isValidElement, type ReactElement, type ReactNode} from "react"

type TitleSegmentKind = "code" | "em" | "text"

interface TitleSegment {
  kind: TitleSegmentKind
  text: string
}

function normalizeInlineText(value: string): string {
  return value.replace(/\s+/g, " ").trim()
}

function collectTitleSegments(
  node: ReactNode,
  inheritedKind: TitleSegmentKind = "text",
): TitleSegment[] {
  const segments: TitleSegment[] = []

  for (const child of Children.toArray(node)) {
    if (typeof child === "string" || typeof child === "number") {
      const text = String(child)
      if (text.trim().length === 0) {
        continue
      }

      segments.push({
        kind: inheritedKind,
        text,
      })
      continue
    }

    if (!isValidElement<{children?: ReactNode}>(child)) {
      continue
    }

    const element = child as ReactElement<{children?: ReactNode}>
    const tagName = typeof element.type === "string" ? element.type : null
    const nextKind = tagName === "code" ? "code" : tagName === "em" ? "em" : inheritedKind

    segments.push(...collectTitleSegments(element.props.children, nextKind))
  }

  const merged: TitleSegment[] = []

  for (const segment of segments) {
    if (segment.kind === "text") {
      const text = segment.text.replace(/\s+/g, " ").trim()
      if (text.length === 0) {
        continue
      }

      const previous = merged[merged.length - 1]
      if (previous?.kind === "text") {
        previous.text = `${previous.text} ${text}`.trim()
      } else {
        merged.push({...segment, text})
      }
      continue
    }

    merged.push({
      ...segment,
      text: normalizeInlineText(segment.text),
    })
  }

  return merged
}

function renderTitleSegment(segment: TitleSegment, index: number) {
  if (segment.kind === "code") {
    return (
      <span
        key={`code-${index}-${segment.text}`}
        className="font-mono text-[0.92rem] font-medium text-fd-primary"
      >
        {segment.text}
      </span>
    )
  }

  if (segment.kind === "em") {
    return (
      <span
        key={`em-${index}-${segment.text}`}
        className="font-mono text-[0.92rem] font-bold text-fd-muted-foreground"
      >
        {`<${segment.text}>`}
      </span>
    )
  }

  const compactText = normalizeInlineText(segment.text)
  const isSeparator = compactText === "," || compactText === "|" || compactText === "/"
  const isBracket = compactText === "[" || compactText === "]" || compactText === "..."

  return (
    <span
      key={`text-${index}-${compactText}`}
      className={
        isSeparator || isBracket
          ? "text-sm font-medium text-fd-muted-foreground/80"
          : "text-sm font-medium text-fd-muted-foreground"
      }
    >
      {compactText}
    </span>
  )
}

export function CommandOptions({children}: {children: ReactNode}) {
  return <div className="my-6 flex flex-col">{children}</div>
}

export function CommandOption({children}: {children: ReactNode}) {
  return (
    <div className="relative py-5 first:pt-1 last:pb-1">
      <div className="pointer-events-none absolute inset-x-0 bottom-0 h-px bg-gradient-to-r from-transparent via-fd-primary/25 to-transparent last:hidden" />
      <div className="relative flex flex-col gap-3 text-[0.95rem] leading-7 text-fd-foreground/90 [&_code]:rounded-md [&_code]:border [&_code]:border-fd-border/70 [&_code]:bg-fd-background/80 [&_code]:px-1.5 [&_code]:py-0.5 [&_code]:text-[0.82em] [&_code]:font-medium [&_code]:text-fd-foreground [&_li+li]:mt-1 [&_p]:m-0 [&_ul]:my-0 [&_ul]:pl-5">
        {children}
      </div>
    </div>
  )
}

export function CommandOptionTitle({children}: {children: ReactNode}) {
  const segments = collectTitleSegments(children)

  return (
    <div className="mb-1 flex flex-col gap-2">
      <div className="flex flex-wrap items-baseline gap-x-2 gap-y-1 text-[0.98rem] font-semibold leading-6 tracking-tight text-fd-foreground">
        {segments.length > 0 ? (
          segments.map(renderTitleSegment)
        ) : (
          <span className="[&_p]:contents">{children}</span>
        )}
      </div>
    </div>
  )
}

export function CommandOptionMeta({label, children}: {label: string; children: ReactNode}) {
  return (
    <div className="flex items-start gap-1.5 rounded-xl bg-fd-primary/5 py-1.5">
      <span className="text-sm leading-6 font-semibold text-fd-primary">{label}:</span>
      <div className="min-w-0 flex-1 text-sm leading-6 text-fd-foreground/85 [&_p]:m-0 [&_code]:rounded-md [&_code]:border-fd-primary/15 [&_code]:bg-fd-background">
        {children}
      </div>
    </div>
  )
}
