import {useEffect, useId, useState, type FC} from "react"
import {Link} from "react-router"

import {useExplorerRoutePaths} from "../hooks/useExplorerRoutePaths"
import styles from "./SuspendedAccountOverview.module.css"

interface SuspendedAccountOverviewProps {
  readonly suspendedUntil: number
}

const MINUTE_SECONDS = 60
const HOUR_SECONDS = 60 * MINUTE_SECONDS
const DAY_SECONDS = 24 * HOUR_SECONDS
const MONTH_SECONDS = 30 * DAY_SECONDS
const YEAR_SECONDS = 365 * DAY_SECONDS

export const SuspendedAccountOverview: FC<SuspendedAccountOverviewProps> = ({suspendedUntil}) => {
  const routes = useExplorerRoutePaths()
  const titleId = useId()
  const [nowSeconds, setNowSeconds] = useState(() => Math.floor(Date.now() / 1000))

  useEffect(() => {
    const interval = globalThis.setInterval(
      () => setNowSeconds(Math.floor(Date.now() / 1000)),
      MINUTE_SECONDS * 1000,
    )
    return () => globalThis.clearInterval(interval)
  }, [])

  const countdown = suspensionCountdown(suspendedUntil, nowSeconds)
  if (suspendedUntil <= nowSeconds) return null

  return (
    <section className={styles.card} aria-labelledby={titleId}>
      <div className={styles.header}>
        <div>
          <h2 id={titleId} className={styles.title}>
            Suspended address
          </h2>
          <p className={styles.description}>
            This address has been suspended through validators&apos; voting
          </p>
        </div>
        <Link
          aria-label="View all suspended addresses"
          className={styles.listLink}
          to={routes.suspendedAddressesPath}
        >
          View all
        </Link>
      </div>

      <div className={styles.divider}>
        <span>Time to unlock</span>
      </div>

      <dl className={styles.countdown}>
        <CountdownValue unit="year" value={countdown.years} />
        <CountdownValue unit="month" value={countdown.months} />
        <CountdownValue unit="day" value={countdown.days} />
        <CountdownValue unit="hour" value={countdown.hours} />
        <CountdownValue unit="minute" value={countdown.minutes} />
      </dl>
    </section>
  )
}

function CountdownValue({unit, value}: {readonly unit: string; readonly value: number}) {
  return (
    <div className={styles.countdownValue}>
      <dt>
        {unit}
        {value === 1 ? "" : "s"}
      </dt>
      <dd>{value}</dd>
    </div>
  )
}

function suspensionCountdown(until: number, now: number) {
  let remaining = Math.max(0, Math.ceil((until - now) / MINUTE_SECONDS)) * MINUTE_SECONDS
  const years = Math.floor(remaining / YEAR_SECONDS)
  remaining %= YEAR_SECONDS
  const months = Math.floor(remaining / MONTH_SECONDS)
  remaining %= MONTH_SECONDS
  const days = Math.floor(remaining / DAY_SECONDS)
  remaining %= DAY_SECONDS
  const hours = Math.floor(remaining / HOUR_SECONDS)
  remaining %= HOUR_SECONDS
  const minutes = Math.floor(remaining / MINUTE_SECONDS)

  return {years, months, days, hours, minutes}
}
