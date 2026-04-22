import * as React from "react"
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
  | {readonly status: "loading"}
  | {readonly status: "ready"; readonly disasm: string}
  | {readonly status: "error"; readonly error: string}

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
      setState(currentState => (currentState.status === "idle" ? currentState : {status: "idle"}))
      return
    }

    let isCancelled = false
    setState({status: "loading"})

    void disassembleBocHex(bocHex)
      .then(result => {
        if (!isCancelled) {
          setState({
            status: "ready",
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
          setState({status: "error", error: message})
        }
      })

    return () => {
      isCancelled = true
    }
  }, [bocHex, isExpanded])

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
          aria-label={isExpanded ? "Hide disassembled code" : "Open disassembled code"}
        >
          {isExpanded ? <FiChevronUp size={14} /> : <FiChevronDown size={14} />}
          <span className={styles.actionsToggleText}>{isExpanded ? "Hide" : "Open"}</span>
        </button>
      </div>

      {isExpanded &&
        (state.status === "loading" ? (
          <div className={styles.disasmLoading}>Loading disassembly...</div>
        ) : state.status === "error" ? (
          <div className={styles.disasmError}>{state.error}</div>
        ) : state.status === "ready" ? (
          <div className={styles.disasmBlock}>
            <pre className={styles.disasmCode}>
              <code>{state.disasm}</code>
            </pre>
          </div>
        ) : (
          <div className={styles.disasmLoading}>Preparing disassembly...</div>
        ))}
    </div>
  )
}
