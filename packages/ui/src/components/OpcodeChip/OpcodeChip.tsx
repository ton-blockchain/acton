import {Check, Copy} from "lucide-react"
import type {ComponentPropsWithRef} from "react"

import {cx} from "../../lib/cx"
import {CopyInlineAction, InlineActions} from "../InlineActions/InlineActions"
import styles from "./OpcodeChip.module.css"

export type OpcodeChipProps = Readonly<
  Omit<ComponentPropsWithRef<"span">, "children"> & {
    readonly opcode?: number | string
    readonly abiName?: string
    readonly showOpcode?: boolean
  }
>

export function OpcodeChip({
  abiName,
  className,
  opcode,
  ref,
  showOpcode = false,
  ...props
}: OpcodeChipProps) {
  const formattedOpcode = formatOpcode(opcode)
  const displayText = abiName ?? formattedOpcode ?? "Empty"
  const displaySubText = abiName && showOpcode ? formattedOpcode : undefined

  return (
    <InlineActions
      {...props}
      ref={ref}
      className={cx(styles.opcodeChip, className)}
      visibility="hover"
      actions={
        formattedOpcode ? (
          <CopyInlineAction
            value={formattedOpcode}
            label={`Copy opcode ${formattedOpcode}`}
            copiedLabel="Opcode copied"
            size="compact"
            icon={<Copy />}
            copiedIcon={<Check />}
          />
        ) : undefined
      }
    >
      <span className={styles.value}>
        <span className={styles.opcodeText}>{displayText}</span>
        {displaySubText ? (
          <span className={styles.opcodeSubText}>· {displaySubText}</span>
        ) : undefined}
      </span>
    </InlineActions>
  )
}

/** Formats an opcode as an unsigned 32-bit hexadecimal value */
export function formatOpcode(opcode: number | string | null | undefined): string | undefined {
  if (opcode === null || opcode === undefined) return undefined
  const normalized = typeof opcode === "string" ? opcode.trim() : opcode
  if (normalized === "") return undefined

  try {
    const value =
      typeof normalized === "number"
        ? Number.isInteger(normalized)
          ? BigInt(normalized)
          : undefined
        : /^[-+]?0x[\da-f]+$/i.test(normalized)
          ? BigInt(normalized.replace(/^\+/, ""))
          : /^[-+]?\d+$/.test(normalized)
            ? BigInt(normalized)
            : undefined
    if (value === undefined || value < -0x8000_0000n || value > 0xffff_ffffn) return undefined
    return `0x${BigInt.asUintN(32, value).toString(16).padStart(8, "0")}`
  } catch {
    return undefined
  }
}
