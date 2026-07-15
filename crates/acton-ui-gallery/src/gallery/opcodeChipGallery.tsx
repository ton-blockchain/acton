import {OpcodeChip} from "@acton/ui"

import styles from "./opcodeChipGallery.module.css"
import type {ComponentGallery} from "./types"

const samples = [
  {
    id: "empty",
    title: "Empty",
    description: "An unavailable opcode renders a stable Empty label without a copy action.",
    chip: <OpcodeChip opcode={undefined} />,
  },
  {
    id: "zero",
    title: "Zero Opcode",
    description: "Zero is a valid opcode and remains copyable as 0x0.",
    chip: <OpcodeChip opcode={0} />,
  },
  {
    id: "numeric",
    title: "Numeric Opcode",
    description: "Unknown opcodes use their lowercase hexadecimal representation.",
    chip: <OpcodeChip opcode={0x73_69_67_6e} />,
  },
  {
    id: "abi-name",
    title: "Resolved ABI Name",
    description: "A resolved ABI name can keep the numeric opcode visible as secondary text.",
    chip: <OpcodeChip opcode={0x73_69_67_6e} abiName="WalletSignedExternalV5r1" showOpcode />,
  },
  {
    id: "abi-name-only",
    title: "ABI Name Only",
    description: "Hide the numeric value when the surrounding layout needs a shorter label.",
    chip: <OpcodeChip opcode={0x73_69_67_6e} abiName="WalletSignedExternalV5r1" />,
  },
] as const

function OpcodeChipSamples() {
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

export const opcodeChipGallery = {
  id: "opcode-chip",
  title: "OpcodeChip",
  status: "ready",
  summary:
    "OpcodeChip renders optional ABI names and hexadecimal opcodes with a shared hover-to-copy action.",
  importStatement: 'import {OpcodeChip} from "@acton/ui"',
  agentSummary:
    "Use OpcodeChip for TON message opcodes. Pass a resolved ABI name from domain code and let the component own hexadecimal formatting and copy feedback.",
  usage: [
    "Pass opcode as a number; zero is treated as the valid value 0x0.",
    "Pass abiName when domain code has resolved a symbolic message name.",
    "Enable showOpcode to retain the hexadecimal value beside an ABI name.",
  ],
  avoid: [
    "Do not format hexadecimal opcode strings in callers.",
    "Do not add a separate copy button around the chip.",
    "Do not resolve ABI names inside the base component.",
  ],
  sections: [
    {
      id: "opcode-chip-states",
      title: "States",
      description: "Hover or focus a populated opcode to reveal its copy action.",
      content: <OpcodeChipSamples />,
    },
  ],
} satisfies ComponentGallery
