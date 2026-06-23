import {memo} from "react"
import type {IconType} from "react-icons"
import {FiCode, FiList} from "react-icons/fi"

import type {TraceViewMode} from "../../lib/traceViewModel"

import styles from "./TraceViewModeToggle.module.css"

const TRACE_VIEW_OPTIONS: ReadonlyArray<{
  readonly value: TraceViewMode
  readonly label: string
  readonly icon: IconType
}> = [
  {value: "assembler", label: "Assembler", icon: FiCode},
  {value: "stepsChain", label: "Steps chain", icon: FiList},
]

interface TraceViewModeToggleProps {
  readonly value: TraceViewMode
  readonly onChange: (value: TraceViewMode) => void
}

function TraceViewModeToggleFc({value, onChange}: TraceViewModeToggleProps) {
  return (
    <div className={styles.root}>
      <div className={styles.toggle} role="group" aria-label="Trace view mode">
        {TRACE_VIEW_OPTIONS.map(option => {
          const isActive = value === option.value
          const Icon = option.icon

          return (
            <button
              key={option.value}
              type="button"
              className={`${styles.button} ${isActive ? styles.buttonActive : ""}`}
              onClick={() => {
                onChange(option.value)
              }}
              aria-pressed={isActive}
            >
              <Icon className={styles.icon} aria-hidden="true" />
              <span>{option.label}</span>
            </button>
          )
        })}
      </div>
    </div>
  )
}

const TraceViewModeToggle = memo(TraceViewModeToggleFc)
TraceViewModeToggle.displayName = "TraceViewModeToggle"

export default TraceViewModeToggle
