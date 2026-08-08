import ReactMarkdown, {type Components} from "react-markdown"
import remarkGfm from "remark-gfm"
import type {ComponentPropsWithRef} from "react"

import {cx} from "../../lib/cx"
import styles from "./MarkdownText.module.css"

export type MarkdownTextTone = "default" | "muted"

export type MarkdownTextProps = Readonly<
  Omit<ComponentPropsWithRef<"div">, "children"> & {
    readonly children: string
    readonly components?: Components
    readonly openLinksInNewTab?: boolean
    readonly tone?: MarkdownTextTone
  }
>

const toneClassNames = {
  default: styles.toneDefault,
  muted: styles.toneMuted,
} satisfies Record<MarkdownTextTone, string>

const baseComponents = {
  code: ({className, children, node: _node, ...props}) => (
    <code {...props} className={cx(styles.inlineCode, className)}>
      {children}
    </code>
  ),
  pre: ({className, node: _node, ...props}) => (
    <pre {...props} className={cx(styles.pre, className)} />
  ),
  table: ({className, node: _node, ...props}) => (
    <div className={styles.tableScroll}>
      <table {...props} className={cx(styles.table, className)} />
    </div>
  ),
} satisfies Components

export function MarkdownText({
  children,
  className,
  components,
  openLinksInNewTab = false,
  ref,
  tone = "default",
  ...props
}: MarkdownTextProps) {
  const defaultComponents = {
    ...baseComponents,
    a: ({className, node: _node, ...props}) => (
      <a
        {...props}
        className={cx(styles.link, className)}
        rel={openLinksInNewTab ? "noreferrer" : undefined}
        target={openLinksInNewTab ? "_blank" : undefined}
      />
    ),
  } satisfies Components

  return (
    <div {...props} ref={ref} className={cx(styles.markdownText, toneClassNames[tone], className)}>
      <ReactMarkdown components={{...defaultComponents, ...components}} remarkPlugins={[remarkGfm]}>
        {children}
      </ReactMarkdown>
    </div>
  )
}
