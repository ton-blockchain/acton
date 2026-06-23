import React, {useMemo, useState} from "react"

import type {ParsedInternal} from "@truecarry/tlb-abi"
import {Address, Cell, ExternalAddress} from "@ton/core"

import {CopyButton} from "@retrace/CopyButton/CopyButton"

import styles from "./ParsedBodyViewer.module.css"

interface ParsedBodyViewerProps {
  readonly parsedBody: ParsedInternal
  readonly cellHex?: string
}

export const ParsedBodyViewer: React.FC<ParsedBodyViewerProps> = ({parsedBody, cellHex}) => {
  const [mode, setMode] = useState<"json" | "yaml">("yaml")

  const content = useMemo(() => {
    const sanitized = sanitizeObject(parsedBody.data)
    return mode === "json" ? JSON.stringify(sanitized, null, 2) : toYaml(sanitized)
  }, [mode, parsedBody.data])

  return (
    <div className={styles.container}>
      <div className={styles.header}>
        <div className={styles.tabs} role="tablist" aria-label="Data format">
          <button
            type="button"
            role="tab"
            aria-selected={mode === "yaml"}
            className={`${styles.tabButton} ${mode === "yaml" ? styles.tabButtonActive : ""}`}
            onClick={() => setMode("yaml")}
          >
            YAML
          </button>
          <button
            type="button"
            role="tab"
            aria-selected={mode === "json"}
            className={`${styles.tabButton} ${mode === "json" ? styles.tabButtonActive : ""}`}
            onClick={() => setMode("json")}
          >
            JSON
          </button>
        </div>
        <div className={styles.actions}>
          <span className={styles.copyAction} role="group">
            <span className={styles.copyText}>{mode === "json" ? "JSON" : "YAML"}</span>
            <CopyButton
              value={content}
              title={mode === "json" ? "Copy JSON" : "Copy YAML"}
              className={styles.copyButton}
            />
          </span>
          {cellHex && (
            <span className={styles.copyAction} role="group">
              <span className={styles.copyText}>Raw hex</span>
              <CopyButton value={cellHex} title="Copy Raw" className={styles.copyButton} />
            </span>
          )}
        </div>
      </div>
      <pre className={styles.codeBlock} data-testid="parsed-body-viewer">
        {content}
      </pre>
    </div>
  )
}

type JsonLike =
  | string
  | number
  | boolean
  | null
  | readonly JsonLike[]
  | Readonly<{[key: string]: JsonLike}>

function sanitizeObject(obj: unknown): JsonLike {
  if (obj instanceof Cell) {
    return obj.toBoc().toString("hex")
  }
  if (obj instanceof Address) {
    return obj.toString()
  }
  if (obj instanceof ExternalAddress) {
    return obj.toString()
  }
  if (obj instanceof Buffer) {
    return obj.toString("hex")
  }
  if (typeof obj === "object" && obj !== null) {
    const record = obj as Record<string, unknown>
    const sanitized: Record<string, JsonLike> = {}
    for (const key in record) {
      if (Object.prototype.hasOwnProperty.call(record, key)) {
        sanitized[key] = sanitizeObject(record[key])
      }
    }
    return sanitized
  }
  if (typeof obj === "bigint") {
    return obj.toString()
  }
  if (
    typeof obj === "string" ||
    typeof obj === "number" ||
    typeof obj === "boolean" ||
    obj === null
  ) {
    return obj
  }
  try {
    return JSON.stringify(obj)
  } catch {
    return "[unserializable]"
  }
}

function toYaml(value: unknown, indent: number = 0): string {
  const pad = (n: number) => " ".repeat(n)

  if (value === null || value === undefined) return "null"
  if (typeof value === "string") return JSON.stringify(value)
  if (typeof value === "number" || typeof value === "bigint") return String(value)
  if (typeof value === "boolean") return value ? "true" : "false"

  if (Array.isArray(value)) {
    if (value.length === 0) return "[]"
    return value.map(item => `${pad(indent)}- ${formatInlineOrBlock(item, indent + 2)}`).join("\n")
  }

  if (typeof value === "object") {
    const entries = Object.entries(value as Record<string, unknown>)
    if (entries.length === 0) return "{}"
    return entries
      .map(([k, v]) => `${pad(indent)}${k}: ${formatInlineOrBlock(v, indent + 2)}`)
      .join("\n")
  }

  return JSON.stringify(value)
}

function formatInlineOrBlock(value: unknown, indent: number): string {
  if (value !== null && typeof value === "object") {
    const isArray = Array.isArray(value)
    const isEmpty = isArray ? (value as unknown[]).length === 0 : Object.keys(value).length === 0
    if (isEmpty) return isArray ? "[]" : "{}"
    const block = toYaml(value, indent)
    const prefix = isArray ? "\n" : "\n"
    return prefix + block
  }
  return toYaml(value, 0)
}
