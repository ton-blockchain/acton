import {Check, Copy} from "lucide-react"
import type {ComponentPropsWithRef} from "react"

import {cx} from "../../lib/cx"
import {CopyInlineAction, InlineActions} from "../InlineActions/InlineActions"
import styles from "./OpcodeChip.module.css"

export type OpcodeChipProps = Readonly<
  Omit<ComponentPropsWithRef<"span">, "children"> & {
    readonly opcode?: number
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
  const formattedOpcode = opcode === undefined ? undefined : `0x${opcode.toString(16)}`
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
