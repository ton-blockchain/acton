import type React from "react"
import styles from "./OpcodeChip.module.css"

interface OpcodeChipProps {
  readonly opcode: number | undefined
  readonly abiName?: string
}

export const OpcodeChip: React.FC<OpcodeChipProps> = ({ opcode, abiName }) => {
  if (opcode === undefined) {
    return <span className={styles.empty}>Empty</span>
  }

  const hexOpcode = `0x${opcode.toString(16).toUpperCase().padStart(8, "0")}`

  return (
    <div className={styles.chip}>
      {abiName && <span className={styles.name}>{abiName}</span>}
      <span className={styles.hex}>{hexOpcode}</span>
    </div>
  )
}
