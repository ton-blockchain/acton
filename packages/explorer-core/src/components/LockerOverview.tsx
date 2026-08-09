import {useEffect, useState, type FC, type ReactNode} from "react"

import {
  Button,
  DataTable,
  DataTableBody,
  DataTableCell,
  DataTableHead,
  DataTableHeaderCell,
  DataTableRow,
  DataTableTable,
  DateTime,
  Dialog,
  formatSchedulePeriod,
  formatTimeUntil,
  GramAmount,
  InlineButton,
  DAY_SECONDS,
  Skeleton,
} from "@acton/ui"

import type {TonClient} from "../api/client"
import {
  buildLockerSchedule,
  parseLockerData,
  type LockerData,
  type LockerPayment,
} from "./lockerSchedule"
import styles from "./LockerOverview.module.css"

const LOCKER_STATUS_LABELS = {
  locked: "Locked",
  next: "Next",
  unlocked: "Unlocked",
} as const

interface LockerOverviewProps {
  readonly address: string
  readonly client: TonClient
}

type LockerLoadState =
  | {readonly status: "loading"}
  | {readonly status: "success"; readonly data: LockerData}
  | {readonly status: "error"; readonly message: string}

export const LockerOverview: FC<LockerOverviewProps> = ({address, client}) => {
  const [loadState, setLoadState] = useState<LockerLoadState>({status: "loading"})
  const [scheduleOpen, setScheduleOpen] = useState(false)
  const [reloadKey, setReloadKey] = useState(0)
  const [nowSeconds, setNowSeconds] = useState(() => Math.floor(Date.now() / 1000))

  useEffect(() => {
    const interval = globalThis.setInterval(
      () => setNowSeconds(Math.floor(Date.now() / 1000)),
      60_000,
    )
    return () => globalThis.clearInterval(interval)
  }, [])

  useEffect(() => {
    let active = true
    const load = async () => {
      setLoadState({status: "loading"})
      try {
        const response = await client.runGetMethod(address, "get_locker_data")
        const data = parseLockerData(response)
        buildLockerSchedule(data, Math.floor(Date.now() / 1000))

        if (active) {
          setLoadState({status: "success", data})
        }
      } catch (error) {
        if (active) {
          setLoadState({
            status: "error",
            message: error instanceof Error ? error.message : String(error),
          })
        }
      }
    }

    void load()
    return () => {
      active = false
    }
  }, [address, client, reloadKey])

  if (loadState.status === "loading") {
    return <LockerOverviewSkeleton />
  }

  if (loadState.status === "error") {
    return (
      <section className={styles.card} aria-label="Locker schedule">
        <div className={styles.error}>
          <div>
            <div className={styles.errorTitle}>Locker details are unavailable</div>
            <div className={styles.errorMessage}>{loadState.message}</div>
          </div>
          <Button
            type="button"
            variant="outline"
            size="sm"
            onClick={() => setReloadKey(key => key + 1)}
          >
            Retry
          </Button>
        </div>
      </section>
    )
  }

  const {data} = loadState
  const schedule = buildLockerSchedule(data, nowSeconds)
  const firstPayment = schedule.payments[0]
  const finalPayment = schedule.payments.at(-1)
  const paymentLabel = data.unlockPeriod === 30 * DAY_SECONDS ? "Monthly payment" : "Payment amount"

  return (
    <>
      <section className={styles.card} aria-labelledby="locker-overview-title">
        <div className={styles.header}>
          <h2 id="locker-overview-title" className={styles.title}>
            Unlock schedule
          </h2>
          <InlineButton
            variant="accent"
            className={styles.scheduleAction}
            onClick={() => setScheduleOpen(true)}
          >
            Payment schedule
          </InlineButton>
          <p className={styles.description}>
            {schedule.totalPeriods} payments every {formatSchedulePeriod(data.unlockPeriod)}, from{" "}
            {firstPayment ? (
              <DateTime display="date-day-month" unit="seconds" value={firstPayment.unlockTime} />
            ) : (
              "—"
            )}{" "}
            to{" "}
            {finalPayment ? (
              <DateTime display="date-day-month" unit="seconds" value={finalPayment.unlockTime} />
            ) : (
              "—"
            )}
          </p>
        </div>

        <div className={styles.metrics}>
          <LockerMetric
            label="Deposit"
            value={
              <GramAmount maximumFractionDigits={2} useGrouping value={data.totalCoinsLocked} />
            }
          />
          <LockerMetric
            label="Reward"
            value={<GramAmount maximumFractionDigits={2} useGrouping value={data.totalReward} />}
          />
          <LockerMetric
            label={paymentLabel}
            value={
              firstPayment ? (
                <GramAmount maximumFractionDigits={2} useGrouping value={firstPayment.amount} />
              ) : (
                "—"
              )
            }
          />
          <LockerMetric
            label="Next payment"
            value={
              schedule.nextPayment ? (
                <DateTime
                  display="date-day-month"
                  unit="seconds"
                  value={schedule.nextPayment.unlockTime}
                />
              ) : (
                "Completed"
              )
            }
          />
        </div>

        <div className={styles.progressSection}>
          <div className={styles.progressHeader}>
            <span className={styles.progressLabel}>Unlocked</span>
            <span className={styles.progressValue}>
              {schedule.unlockedPeriods} of {schedule.totalPeriods} payments
            </span>
          </div>
          <div
            className={styles.progressSegments}
            role="progressbar"
            aria-label="Unlocked locker payments"
            aria-valuemin={0}
            aria-valuemax={schedule.totalPeriods}
            aria-valuenow={schedule.unlockedPeriods}
            style={{
              gridTemplateColumns: `repeat(${schedule.totalPeriods}, minmax(0, 1fr))`,
            }}
          >
            {schedule.payments.map(payment => (
              <span
                key={payment.number}
                className={`${styles.progressSegment} ${styles[`progressSegment${LOCKER_STATUS_LABELS[payment.status]}`]}`}
                title={`Payment ${payment.number}: ${LOCKER_STATUS_LABELS[payment.status]}`}
                aria-hidden="true"
              />
            ))}
          </div>
          <div className={styles.progressMeta}>
            <span>
              <GramAmount maximumFractionDigits={2} useGrouping value={schedule.unlockedAmount} />{" "}
              unlocked
            </span>
            <span>
              {schedule.nextPayment
                ? `Next payment ${formatTimeUntil(schedule.nextPayment.unlockTime, nowSeconds)}`
                : "All payments unlocked"}
            </span>
          </div>
        </div>
      </section>

      <Dialog
        open={scheduleOpen}
        onOpenChange={setScheduleOpen}
        title="Payment schedule"
        description={`${schedule.totalPeriods} scheduled unlocks`}
        closeLabel="Close payment schedule"
        maxWidth="58rem"
        contentClassName={styles.dialogContent}
      >
        <DataTable minWidth="44rem">
          <DataTableTable aria-label="Locker payment schedule">
            <DataTableHead>
              <DataTableRow>
                <DataTableHeaderCell columnWidth="4rem">#</DataTableHeaderCell>
                <DataTableHeaderCell>Unlock date</DataTableHeaderCell>
                <DataTableHeaderCell align="right">Payment</DataTableHeaderCell>
                <DataTableHeaderCell align="right">Cumulative</DataTableHeaderCell>
                <DataTableHeaderCell columnWidth="6rem">Status</DataTableHeaderCell>
              </DataTableRow>
            </DataTableHead>
            <DataTableBody>
              {schedule.payments.map(payment => (
                <LockerPaymentRow key={payment.number} payment={payment} />
              ))}
            </DataTableBody>
          </DataTableTable>
        </DataTable>
      </Dialog>
    </>
  )
}

function LockerMetric({label, value}: {readonly label: string; readonly value: ReactNode}) {
  return (
    <div className={styles.metric}>
      <div className={styles.metricLabel}>{label}</div>
      <div className={styles.metricValue}>{value}</div>
    </div>
  )
}

function LockerPaymentRow({payment}: {readonly payment: LockerPayment}) {
  const statusLabel = LOCKER_STATUS_LABELS[payment.status]

  return (
    <DataTableRow selected={payment.status === "next"}>
      <DataTableCell tone="muted">{payment.number}</DataTableCell>
      <DataTableCell>
        <DateTime display="date-day-month" unit="seconds" value={payment.unlockTime} />
      </DataTableCell>
      <DataTableCell align="right" tone="strong">
        <GramAmount maximumFractionDigits={2} useGrouping value={payment.amount} />
      </DataTableCell>
      <DataTableCell align="right">
        <GramAmount maximumFractionDigits={2} useGrouping value={payment.cumulativeAmount} />
      </DataTableCell>
      <DataTableCell>
        <span className={`${styles.status} ${styles[`status${statusLabel}`]}`}>{statusLabel}</span>
      </DataTableCell>
    </DataTableRow>
  )
}

function LockerOverviewSkeleton() {
  return (
    <section className={styles.card} aria-label="Loading locker schedule" aria-busy="true">
      <div className={styles.header}>
        <Skeleton width="9rem" />
        <Skeleton width="8.5rem" height="2rem" radius="md" />
        <div className={styles.description}>
          <Skeleton width="100%" />
        </div>
      </div>
      <div className={styles.metrics}>
        {Array.from({length: 4}, (_, index) => (
          <div className={styles.metric} key={index}>
            <Skeleton width="5rem" />
            <Skeleton width="9rem" />
          </div>
        ))}
      </div>
      <div className={styles.progressSection}>
        <Skeleton width="100%" height="0.5rem" radius="md" />
      </div>
    </section>
  )
}
