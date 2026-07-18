import type {MouseEventHandler, ReactNode} from "react"

import {cx} from "../../lib/cx"
import {CopyInlineAction, InlineActions} from "../InlineActions/InlineActions"

import styles from "./BlockChip.module.css"

export interface BlockChipProps {
  readonly workchain: number
  readonly shard: string
  readonly seqno: number | string
  readonly className?: string
  readonly highlighted?: boolean
  readonly href?: string
  readonly label?: ReactNode
  readonly onClick?: MouseEventHandler<HTMLAnchorElement>
  readonly title?: string
}

export function BlockChip({
  workchain,
  shard,
  seqno,
  className,
  highlighted = false,
  href,
  label,
  onClick,
  title,
}: BlockChipProps) {
  const content = label ?? seqno
  const chipClassName = cx(styles.blockChip, highlighted && styles.highlighted, className)
  const chipTitle = title ?? `Block ${seqno}`
  const toncenterBlockId = `(${workchain},${shard},${seqno})`

  return (
    <InlineActions
      className={styles.blockCluster}
      visibility="hover"
      actions={
        <CopyInlineAction
          value={toncenterBlockId}
          label="Copy block ID"
          copiedLabel="Block ID copied"
        />
      }
    >
      {href ? (
        <a className={chipClassName} href={href} onClick={onClick} title={chipTitle}>
          {content}
        </a>
      ) : (
        <span className={chipClassName} title={chipTitle}>
          {content}
        </span>
      )}
    </InlineActions>
  )
}
