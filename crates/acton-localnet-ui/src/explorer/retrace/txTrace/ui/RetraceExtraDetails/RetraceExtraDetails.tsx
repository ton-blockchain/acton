import {useMemo} from "react"
import {beginCell, Cell, loadTransaction, storeMessage} from "@ton/core"

import {DataBlock} from "@acton/shared-ui"

import type {RetraceResultAndCode} from "@retrace/txTrace/lib/types"

import styles from "./RetraceExtraDetails.module.css"

interface RetraceExtraDetailsProps {
  readonly result: RetraceResultAndCode
}

function formatDetailedDate(value: string | undefined): string {
  if (!value) return "—"

  const date = new Date(value)
  if (Number.isNaN(date.getTime())) return value

  return date.toLocaleString()
}

export function RetraceExtraDetails({result}: RetraceExtraDetailsProps): React.JSX.Element | null {
  const messageHex = useMemo(() => {
    const rawTransaction = result.result.emulatedTx.raw
    if (!rawTransaction) return undefined

    try {
      const transaction = loadTransaction(Cell.fromHex(rawTransaction).asSlice())
      if (!transaction.inMessage) return undefined

      return beginCell().store(storeMessage(transaction.inMessage)).endCell().toBoc().toString("hex")
    } catch {
      return undefined
    }
  }, [result.result.emulatedTx.raw])

  const emulatorVersion = result.result.emulatorVersion

  if (!messageHex && !emulatorVersion) {
    return null
  }

  return (
    <div className={styles.container}>
      {emulatorVersion && (
        <section className={styles.section}>
          <div className={styles.sectionTitle}>Retrace</div>
          <div className={styles.sectionContent}>
            <div className={styles.multiColumnRow}>
              <div className={styles.multiColumnItem}>
                <div className={styles.multiColumnItemTitle}>Emulator Version</div>
                <div className={styles.multiColumnItemValue}>
                  {emulatorVersion.commitHash.substring(0, 7)}
                </div>
              </div>
              <div className={styles.multiColumnItem}>
                <div className={styles.multiColumnItemTitle}>Emulator Date</div>
                <div className={styles.multiColumnItemValue}>
                  {formatDetailedDate(emulatorVersion.commitDate)}
                </div>
              </div>
            </div>
          </div>
        </section>
      )}

      {messageHex && (
        <section className={styles.section}>
          <div className={styles.sectionTitle}>Raw In Message</div>
          <div className={styles.sectionContent}>
            <DataBlock data={messageHex} />
          </div>
        </section>
      )}
    </div>
  )
}
