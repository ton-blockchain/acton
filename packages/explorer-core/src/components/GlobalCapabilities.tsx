import {ModeViewer, NumberValue, Popover, type ModeInfo, type ModeParser} from "@acton/ui"
import type {FC} from "react"

import styles from "./GlobalCapabilities.module.css"

const GLOBAL_CAPABILITIES = [
  {
    value: 1,
    name: "capIhrEnabled",
    description: "Enables Instant Hypercube Routing",
  },
  {
    value: 2,
    name: "capCreateStatsEnabled",
    description: "Enables creation statistics in the masterchain state",
  },
  {
    value: 4,
    name: "capBounceMsgBody",
    description: "Allows bounced messages to retain part of the original message body",
  },
  {
    value: 8,
    name: "capReportVersion",
    description: "Makes collators report their supported version and capabilities in blocks",
  },
  {
    value: 16,
    name: "capSplitMergeTransactions",
    description: "Enables shard split and merge transactions",
  },
  {
    value: 32,
    name: "capShortDequeue",
    description: "Enables short dequeue records in block message queues",
  },
  {
    value: 64,
    name: "capStoreOutMsgQueueSize",
    description: "Stores the outgoing message queue size in the shard state",
  },
  {
    value: 128,
    name: "capMsgMetadata",
    description: "Enables transaction-chain metadata in message envelopes",
  },
  {
    value: 256,
    name: "capDeferMessages",
    description: "Enables deferred message processing through dispatch queues",
  },
  {
    value: 512,
    name: "capFullCollatedData",
    description: "Enables full collated data for block validation",
  },
] as const

const parseGlobalCapabilities: ModeParser = mode => {
  const flags: ModeInfo[] = GLOBAL_CAPABILITIES.filter(
    capability => Math.floor(mode / capability.value) % 2 === 1,
  ).map(capability => ({...capability}))
  const knownValue = flags.reduce((sum, flag) => sum + flag.value, 0)
  const unknownValue = mode - knownValue

  if (unknownValue > 0) {
    flags.push({
      value: unknownValue,
      name: "Unknown capabilities",
      description: "Capability bits that are not known to this explorer version",
    })
  } else if (mode === 0) {
    flags.push({
      value: 0,
      name: "No capabilities",
      description: "This block does not report any enabled global capabilities",
    })
  }

  return flags
}

export const GlobalCapabilities: FC<{readonly value: bigint | number | string}> = ({value}) => {
  const mode = Number(value)
  if (!Number.isSafeInteger(mode) || mode < 0) {
    return <>{value}</>
  }

  return (
    <Popover
      ariaLabel={`Explain global capabilities ${value}`}
      content={
        <span className={styles.popover}>
          <span className={styles.popoverTitle}>Enabled capabilities</span>
          <ModeViewer mode={mode} parseMode={parseGlobalCapabilities} />
        </span>
      }
      interaction="click"
      placement="top"
      maxWidth="min(42rem, calc(100vw - 2rem))"
    >
      <button type="button" className={styles.trigger}>
        <NumberValue value={value} />
      </button>
    </Popover>
  )
}
