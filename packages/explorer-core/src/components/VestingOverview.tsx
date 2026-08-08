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
  Skeleton,
} from "@acton/ui"

import type {TonClient} from "../api/client"
import styles from "./LockerOverview.module.css"
import {
  buildVestingSchedule,
  parseVestingData,
  type VestingData,
  type VestingPeriod,
} from "./vestingSchedule"

const VESTING_STATUS_LABELS = {
  locked: "Locked",
  next: "Next",
  unlocked: "Unlocked",
} as const

interface VestingOverviewProps {
  readonly address: string
  readonly client: TonClient
  readonly onDataChange?: (data: VestingData | undefined) => void
}

type VestingLoadState =
  | {readonly status: "loading"}
  | {readonly status: "success"; readonly data: VestingData}
  | {readonly status: "error"; readonly message: string}

export const VestingOverview: FC<VestingOverviewProps> = ({address, client, onDataChange}) => {
  const [loadState, setLoadState] = useState<VestingLoadState>({status: "loading"})
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
      onDataChange?.(undefined)
      try {
        const response = await client.runGetMethod(address, "get_vesting_data")
        const data = parseVestingData(response)
        buildVestingSchedule(data, Math.floor(Date.now() / 1000))

        if (active) {
          setLoadState({status: "success", data})
          onDataChange?.(data)
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
  }, [address, client, onDataChange, reloadKey])

  if (loadState.status === "loading") {
    return <VestingOverviewSkeleton />
  }

  if (loadState.status === "error") {
    return (
      <section className={styles.card} aria-label="Vesting schedule">
        <div className={styles.error}>
          <div>
            <div className={styles.errorTitle}>Vesting details are unavailable</div>
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
  const schedule = buildVestingSchedule(data, nowSeconds)
  const cliffEndTime = data.vestingStartTime + data.cliffDuration
  const vestingEndTime = data.vestingStartTime + data.vestingTotalDuration

  return (
    <>
      <section className={styles.card} aria-labelledby="vesting-overview-title">
        <div className={styles.header}>
          <h2 id="vesting-overview-title" className={styles.title}>
            Vesting schedule
          </h2>
          <InlineButton
            variant="accent"
            className={styles.scheduleAction}
            onClick={() => setScheduleOpen(true)}
          >
            Payment schedule
          </InlineButton>
          <p className={styles.description}>
            {schedule.totalPeriods} periods over {formatSchedulePeriod(data.vestingTotalDuration)},
            from <DateTime display="date-day-month" unit="seconds" value={data.vestingStartTime} />{" "}
            to <DateTime display="date-day-month" unit="seconds" value={vestingEndTime} />
            {"; cliff ends "}
            <DateTime display="date-day-month" unit="seconds" value={cliffEndTime} />.
          </p>
        </div>

        <div className={styles.metrics}>
          <VestingMetric
            label="Total vested"
            value={
              <GramAmount maximumFractionDigits={2} useGrouping value={data.vestingTotalAmount} />
            }
          />
          <VestingMetric
            label="Unlocked"
            value={
              <GramAmount maximumFractionDigits={2} useGrouping value={schedule.unlockedAmount} />
            }
          />
          <VestingMetric label="Cliff period" value={formatSchedulePeriod(data.cliffDuration)} />
          <VestingMetric label="Unlock period" value={formatSchedulePeriod(data.unlockPeriod)} />
        </div>

        <div className={styles.progressSection}>
          <div className={styles.progressHeader}>
            <span className={styles.progressLabel}>Unlocked</span>
            <span className={styles.progressValue}>
              {schedule.unlockedPeriods} of {schedule.totalPeriods} periods
            </span>
          </div>
          <div
            className={styles.progressSegments}
            role="progressbar"
            aria-label="Unlocked vesting periods"
            aria-valuemin={0}
            aria-valuemax={schedule.totalPeriods}
            aria-valuenow={schedule.unlockedPeriods}
            style={{
              gridTemplateColumns: `repeat(${schedule.totalPeriods}, minmax(0, 1fr))`,
            }}
          >
            {schedule.periods.map(period => (
              <span
                key={period.number}
                className={`${styles.progressSegment} ${styles[`progressSegment${VESTING_STATUS_LABELS[period.status]}`]}`}
                title={`Period ${period.number}: ${VESTING_STATUS_LABELS[period.status]}`}
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
              {schedule.nextPayoutTime
                ? `Next unlock ${formatTimeUntil(schedule.nextPayoutTime, nowSeconds)}`
                : "All funds unlocked"}
            </span>
          </div>
        </div>
      </section>

      <Dialog
        open={scheduleOpen}
        onOpenChange={setScheduleOpen}
        title="Payment schedule"
        description={`${schedule.totalPeriods} vesting periods`}
        closeLabel="Close payment schedule"
        maxWidth="64rem"
        contentClassName={styles.dialogContent}
      >
        <DataTable minWidth="52rem">
          <DataTableTable aria-label="Vesting payment schedule">
            <DataTableHead>
              <DataTableRow>
                <DataTableHeaderCell columnWidth="4rem">#</DataTableHeaderCell>
                <DataTableHeaderCell>Accrual starts</DataTableHeaderCell>
                <DataTableHeaderCell>Available</DataTableHeaderCell>
                <DataTableHeaderCell align="right">Amount</DataTableHeaderCell>
                <DataTableHeaderCell align="right">Cumulative</DataTableHeaderCell>
                <DataTableHeaderCell columnWidth="6rem">Status</DataTableHeaderCell>
              </DataTableRow>
            </DataTableHead>
            <DataTableBody>
              {schedule.periods.map(period => (
                <VestingPeriodRow key={period.number} period={period} />
              ))}
            </DataTableBody>
          </DataTableTable>
        </DataTable>
      </Dialog>
    </>
  )
}

function VestingMetric({label, value}: {readonly label: string; readonly value: ReactNode}) {
  return (
    <div className={styles.metric}>
      <div className={styles.metricLabel}>{label}</div>
      <div className={styles.metricValue}>{value}</div>
    </div>
  )
}

function VestingPeriodRow({period}: {readonly period: VestingPeriod}) {
  const statusLabel = VESTING_STATUS_LABELS[period.status]

  return (
    <DataTableRow selected={period.status === "next"}>
      <DataTableCell tone="muted">{period.number}</DataTableCell>
      <DataTableCell>
        <DateTime display="date-day-month" unit="seconds" value={period.startTime} />
      </DataTableCell>
      <DataTableCell>
        <DateTime display="date-day-month" unit="seconds" value={period.payoutTime} />
      </DataTableCell>
      <DataTableCell align="right" tone="strong">
        <GramAmount maximumFractionDigits={2} useGrouping value={period.amount} />
      </DataTableCell>
      <DataTableCell align="right">
        <GramAmount maximumFractionDigits={2} useGrouping value={period.cumulativeAmount} />
      </DataTableCell>
      <DataTableCell>
        <span className={`${styles.status} ${styles[`status${statusLabel}`]}`}>{statusLabel}</span>
      </DataTableCell>
    </DataTableRow>
  )
}

function VestingOverviewSkeleton() {
  return (
    <section className={styles.card} aria-label="Loading vesting schedule" aria-busy="true">
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
