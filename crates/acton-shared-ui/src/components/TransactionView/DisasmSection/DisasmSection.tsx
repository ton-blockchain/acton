import type * as React from "react"
import {useEffect, useState} from "react"
import {FiChevronDown, FiChevronUp} from "react-icons/fi"

import {disassembleBocHex} from "@/utils/disasm"

import styles from "./DisasmSection.module.css"

interface DisasmSectionProps {
  readonly bocHex: string
  readonly title?: string
  readonly defaultExpanded?: boolean
}

type DisasmState =
  | {readonly status: "idle"}
  | {readonly status: "ready"; readonly bocHex: string; readonly disasm: string}
  | {readonly status: "error"; readonly bocHex: string; readonly error: string}

const EMPTY_DISASM_TEXT = "// Disassembly is empty for this code cell"
const EMPTY_CELL_DISASM_TEXT = "// Cell is empty"

export function DisasmSection({
  bocHex,
  title = "Disassembled Code",
  defaultExpanded = false,
}: DisasmSectionProps): React.JSX.Element {
  const [isExpanded, setIsExpanded] = useState(defaultExpanded)
  const [state, setState] = useState<DisasmState>({status: "idle"})

  useEffect(() => {
    if (!isExpanded) {
      return
    }

    if ((state.status === "ready" || state.status === "error") && state.bocHex === bocHex) {
      return
    }

    let isCancelled = false

    void disassembleBocHex(bocHex)
      .then(result => {
        if (!isCancelled) {
          setState({
            status: "ready",
            bocHex,
            disasm:
              result.disasm.trim().length > 0
                ? result.disasm
                : result.isEmptyCell
                  ? EMPTY_CELL_DISASM_TEXT
                  : EMPTY_DISASM_TEXT,
          })
        }
      })
      .catch(error => {
        if (!isCancelled) {
          const message = error instanceof Error ? error.message : "Failed to disassemble code"
          setState({status: "error", bocHex, error: message})
        }
      })

    return () => {
      isCancelled = true
    }
  }, [bocHex, isExpanded, state])

  return (
    <div className={styles.disasmSection}>
      <div className={styles.disasmTitle}>
        {title}
        <button
          type="button"
          onClick={() => {
            setIsExpanded(!isExpanded)
          }}
          className={styles.actionsToggleButton}
          aria-label={isExpanded ? "Hide disassembled code" : "Show disassembled code"}
        >
          {isExpanded ? <FiChevronUp size={14} /> : <FiChevronDown size={14} />}
          <span className={styles.actionsToggleText}>{isExpanded ? "Hide" : "Show"}</span>
        </button>
      </div>

      {isExpanded &&
        (state.status === "error" && state.bocHex === bocHex ? (
          <div className={styles.disasmError}>{state.error}</div>
        ) : state.status === "ready" && state.bocHex === bocHex ? (
          <div className={styles.disasmBlock}>
            <pre className={styles.disasmCode}>
              <code>{state.disasm}</code>
            </pre>
          </div>
        ) : undefined)}
    </div>
  )
}
