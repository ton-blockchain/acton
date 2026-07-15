import {ExitCodeChip, type ExitCodeAbi} from "@acton/ui"

import styles from "./exitCodeChipGallery.module.css"
import type {ComponentGallery} from "./types"

const sampleAbi = {
  thrown_errors: [
    {
      err_code: 1000,
      name: "Unauthorized",
      description: "The sender is not allowed to perform this operation.",
    },
  ],
} satisfies ExitCodeAbi

const samples = [
  {
    id: "empty",
    title: "Unavailable",
    description: "Missing exit code renders a yellow Unknown chip without a popover.",
    chip: <ExitCodeChip exitCode={undefined} />,
  },
  {
    id: "compute-success",
    title: "Compute Success",
    description: "Compute exit codes 0 and 1 are successful.",
    chip: <ExitCodeChip exitCode={1} />,
  },
  {
    id: "compute-error",
    title: "Standard Compute Error",
    description: "Known TVM codes include their official name and documentation link.",
    chip: <ExitCodeChip exitCode={-14} />,
  },
  {
    id: "action-error",
    title: "Standard Action Error",
    description: "Action-phase failures use the action-specific origin in the popover.",
    chip: <ExitCodeChip exitCode={32} phase="action" />,
  },
  {
    id: "abi-error",
    title: "ABI Error",
    description: "A minimal thrown_errors entry supplies the symbolic name and description.",
    chip: <ExitCodeChip exitCode={1000} abi={sampleAbi} />,
  },
  {
    id: "unknown-error",
    title: "Unknown Custom Error",
    description: "Undeclared codes retain a useful fallback instead of an empty label.",
    chip: <ExitCodeChip exitCode={700} />,
  },
] as const

function ExitCodeSamples() {
  return (
    <div className={styles.grid}>
      {samples.map(sample => (
        <article key={sample.id} className={styles.sample}>
          <div className={styles.sampleText}>
            <h4 className={styles.sampleTitle}>{sample.title}</h4>
            <p className={styles.sampleDescription}>{sample.description}</p>
          </div>
          {sample.chip}
        </article>
      ))}
    </div>
  )
}

export const exitCodeChipGallery = {
  id: "exit-code-chip",
  title: "ExitCodeChip",
  status: "ready",
  summary:
    "ExitCodeChip renders compute and action exit codes with success or error styling and contextual TVM or ABI details.",
  importStatement: 'import {ExitCodeChip} from "@acton/ui"',
  agentSummary:
    "Use ExitCodeChip for TVM compute and action result codes. Pass only an ABI-shaped object with thrown_errors when contract-defined names are available.",
  usage: [
    'Use phase="action" for action result codes so success rules and origin labels are correct.',
    "Pass abi only when contract-defined thrown errors should resolve to symbolic names.",
    "Use undefined while the code is unavailable; the component renders an em dash.",
  ],
  avoid: [
    "Do not import a full ABI package only to satisfy this component; its ExitCodeAbi type is intentionally structural and minimal.",
    "Do not manually duplicate TVM code names or success rules in callers.",
    "Do not use the chip for non-TVM application status codes.",
  ],
  sections: [
    {
      id: "exit-code-chip-states",
      title: "States",
      description:
        "Hover or focus a populated chip to inspect its description, execution phase, and documentation link when available.",
      content: <ExitCodeSamples />,
    },
  ],
} satisfies ComponentGallery
