import type {MouseEventHandler, ReactNode} from "react"

import {cx} from "../../lib/cx"
import {CopyInlineAction, InlineActions} from "../InlineActions/InlineActions"
import {Tooltip} from "../Tooltip/Tooltip"

import {formatToncenterBlockId} from "./blockId"
import styles from "./BlockChip.module.css"

import type {ToncenterBlockId} from "./blockId"

export interface BlockChipProps extends ToncenterBlockId {
  readonly className?: string
  readonly copyable?: boolean
  readonly display?: "seqno" | "full"
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
  copyable = true,
  display = "seqno",
  highlighted = false,
  href,
  label,
  onClick,
  title,
}: BlockChipProps) {
  const toncenterBlockId = formatToncenterBlockId({workchain, shard, seqno})
  const content = label ?? (display === "full" ? toncenterBlockId : seqno)
  const chipClassName = cx(
    styles.blockChip,
    display === "full" ? styles.fullBlockId : styles.blockSeqno,
    highlighted && styles.highlighted,
    className,
  )
  const chip = href ? (
    <a className={chipClassName} href={href} onClick={onClick} aria-label={title}>
      {content}
    </a>
  ) : (
    <span className={chipClassName}>{content}</span>
  )
  const chipWithTooltip = (
    <Tooltip
      content={
        <BlockChipTooltip
          blockId={toncenterBlockId}
          heading={title}
          seqno={seqno}
          shard={shard}
          workchain={workchain}
        />
      }
      width="wide"
    >
      {chip}
    </Tooltip>
  )

  if (!copyable) {
    return chipWithTooltip
  }

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
      {chipWithTooltip}
    </InlineActions>
  )
}

function BlockChipTooltip({
  blockId,
  heading,
  seqno,
  shard,
  workchain,
}: {
  readonly blockId: string
  readonly heading?: string
  readonly seqno: number | string
  readonly shard: string
  readonly workchain: number
}) {
  return (
    <span className={styles.tooltip}>
      {heading ? <strong>{heading}</strong> : null}
      <BlockChipTooltipRow copyLabel="block ID" label="Block ID" value={blockId} />
      <BlockChipTooltipRow copyLabel="workchain" label="Workchain" value={workchain.toString()} />
      <BlockChipTooltipRow copyLabel="shard" label="Shard" value={shard} />
      <BlockChipTooltipRow copyLabel="seqno" label="Seqno" value={seqno.toString()} />
    </span>
  )
}

function BlockChipTooltipRow({
  copyLabel,
  label,
  value,
}: {
  readonly copyLabel: string
  readonly label: string
  readonly value: string
}) {
  return (
    <span className={styles.tooltipRow}>
      <span>{label}</span>
      <span className={styles.tooltipCopyValue}>
        <code>{value}</code>
        <CopyInlineAction
          copiedLabel={`${copyLabel} copied`}
          label={`Copy ${copyLabel}`}
          size="compact"
          value={value}
        />
      </span>
    </span>
  )
}
