import {PillTab, PillTabs} from "@acton/ui"
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
      <span className={styles.label}>View mode:</span>
      <PillTabs className={styles.toggle} ariaLabel="Trace view mode">
        {TRACE_VIEW_OPTIONS.map(option => {
          const isActive = value === option.value
          const Icon = option.icon

          return (
            <PillTab
              key={option.value}
              className={styles.button}
              selected={isActive}
              onClick={() => {
                onChange(option.value)
              }}
            >
              <Icon className={styles.icon} aria-hidden="true" />
              <span>{option.label}</span>
            </PillTab>
          )
        })}
      </PillTabs>
    </div>
  )
}

const TraceViewModeToggle = memo(TraceViewModeToggleFc)
TraceViewModeToggle.displayName = "TraceViewModeToggle"

export default TraceViewModeToggle
