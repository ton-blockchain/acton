import {MarkdownText} from "@acton/ui"

import styles from "./markdownTextGallery.module.css"
import type {ComponentGallery} from "./types"

const inlineMarkdown = [
  "Render short docs with **strong emphasis**, _quiet emphasis_,",
  "`inline code`, and [internal links](#markdown-text).",
].join(" ")

const technicalNoteMarkdown = [
  "### Trace Render Contract",
  "",
  "- Keep route-specific values outside the component.",
  "- Use `Breadcrumbs` for hierarchy and `RawDataBlock` for logs.",
  "- Preserve known labels while unresolved parts are loading.",
  "",
  "- [x] Known path segments stay visible",
  "- [ ] Error copy is owned by the caller",
].join("\n")

const codeBlockMarkdown = [
  "```tsx",
  'import {MarkdownText} from "@acton/ui"',
  "",
  "<MarkdownText>",
  "  {noteMarkdown}",
  "</MarkdownText>",
  "```",
].join("\n")

const tableMarkdown = [
  "| Primitive | Use For |",
  "| --- | --- |",
  "| `MarkdownText` | Trusted markdown prose and technical notes |",
  "| `RawDataBlock` | Raw payloads, VM logs, base64, hex, and disasm |",
  "| `DataTable` | Structured rows and values |",
].join("\n")

function MarkdownSamples() {
  return (
    <div className={styles.sampleGrid}>
      <article className={styles.samplePanel}>
        <h4 className={styles.sampleTitle}>Inline Formatting</h4>
        <MarkdownText>{inlineMarkdown}</MarkdownText>
      </article>

      <article className={styles.samplePanel}>
        <h4 className={styles.sampleTitle}>Muted Technical Note</h4>
        <MarkdownText tone="muted">{technicalNoteMarkdown}</MarkdownText>
      </article>

      <article className={styles.samplePanel}>
        <h4 className={styles.sampleTitle}>Code Fence</h4>
        <MarkdownText>{codeBlockMarkdown}</MarkdownText>
      </article>

      <article className={styles.samplePanel}>
        <h4 className={styles.sampleTitle}>GFM Table</h4>
        <MarkdownText>{tableMarkdown}</MarkdownText>
      </article>
    </div>
  )
}

function LinkPolicySample() {
  return (
    <div className={styles.sampleGrid}>
      <article className={styles.samplePanel}>
        <h4 className={styles.sampleTitle}>Default Links</h4>
        <MarkdownText>
          {"Links render as normal anchors by default: [Acton UI](#markdown-text-link-policy)."}
        </MarkdownText>
      </article>

      <article className={styles.samplePanel}>
        <h4 className={styles.sampleTitle}>External Links</h4>
        <MarkdownText openLinksInNewTab>
          {"Set `openLinksInNewTab` when markdown points outside the current app."}
        </MarkdownText>
      </article>
    </div>
  )
}

export const markdownTextGallery = {
  id: "markdown-text",
  title: "MarkdownText",
  status: "ready",
  summary:
    "MarkdownText renders trusted markdown prose with shared typography, inline code, links, lists, code blocks, and GFM tables.",
  importStatement: 'import {MarkdownText} from "@acton/ui"',
  agentSummary:
    "Use MarkdownText for trusted markdown descriptions, help text, release notes, and agent-facing UI notes. Use RawDataBlock for raw logs, payloads, and code viewers.",
  usage: [
    "Use for markdown prose from trusted product copy, docs, or generated UI notes.",
    "Use when text can contain `inline code`, links, lists, code fences, task lists, or markdown tables.",
    'Use tone="muted" for secondary explanatory text that still needs markdown support.',
    "Use components overrides only when a screen needs a custom renderer for a known markdown element.",
  ],
  avoid: [
    "Do not use for untrusted user-authored markdown without a sanitization policy at the caller boundary.",
    "Do not use for raw payloads, VM logs, base64, hex, or disassembly; use RawDataBlock.",
    "Do not hand-parse backticks or links in app code when MarkdownText fits.",
  ],
  sections: [
    {
      id: "markdown-text-formats",
      title: "Markdown Formats",
      description:
        "Shared rendering for prose, inline code, lists, task lists, code fences, and GFM tables.",
      content: <MarkdownSamples />,
    },
    {
      id: "markdown-text-link-policy",
      title: "Link Policy",
      description:
        "Links stay in the current browsing context unless callers explicitly opt into new tabs.",
      content: <LinkPolicySample />,
    },
  ],
} satisfies ComponentGallery
