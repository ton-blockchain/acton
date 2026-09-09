import {useEffect, useState, type MouseEvent, type ReactNode} from "react"

import {
  Button,
  DataTable,
  DataTableBody,
  DataTableCell,
  DataTableEmpty,
  DataTableHead,
  DataTableHeaderCell,
  DataTableRow,
  DataTableSkeletonRows,
  DataTableTable,
  GramAmount,
  Skeleton,
} from "@acton/ui"

import type {TonClient} from "../api/client"
import type {SingleNominatorRoles, V3NominatorPool} from "../api/types"
import {useAddressBook} from "../hooks/useAddressBook"
import {ExplorerAddressChip} from "./ExplorerAddressChip"
import styles from "./StakingPoolDetails.module.css"

/** Shared load state keeps the pool overview and its Nominators tab on one API snapshot. */
export type NominatorPoolDetailsState =
  | {readonly status: "loading"}
  | {readonly status: "success"; readonly data: V3NominatorPool}
  | {readonly status: "error"; readonly message: string}

interface NominatorPoolOverviewProps {
  readonly address: string
  readonly client: TonClient
  readonly onAddressClick?: (address: string, event?: MouseEvent<HTMLElement>) => void
  readonly onStateChange: (state: NominatorPoolDetailsState) => void
}

interface NominatorPoolNominatorsTabProps {
  readonly state: NominatorPoolDetailsState
  readonly onAddressClick?: (address: string, event?: MouseEvent<HTMLElement>) => void
}

interface SingleNominatorOverviewProps {
  readonly address: string
  readonly client: TonClient
  readonly onAddressClick?: (address: string, event?: MouseEvent<HTMLElement>) => void
  readonly version: string
}

type SingleNominatorLoadState =
  | {readonly status: "loading"}
  | {readonly status: "success"; readonly data: SingleNominatorRoles}
  | {readonly status: "error"; readonly message: string}

const ONE_GRAM = 1_000_000_000n

export function NominatorPoolOverview({
  address,
  client,
  onAddressClick,
  onStateChange,
}: NominatorPoolOverviewProps) {
  const [state, setState] = useState<NominatorPoolDetailsState>({status: "loading"})
  const [reloadKey, setReloadKey] = useState(0)
  const {updateDomains} = useAddressBook()

  useEffect(() => {
    let active = true
    const nextState = {status: "loading"} as const
    setState(nextState)
    onStateChange(nextState)

    void client
      .getNominatorPool(address)
      .then(data => {
        if (!active) return

        updateDomains(data.address_book)
        const successState = {status: "success", data} as const
        setState(successState)
        onStateChange(successState)
      })
      .catch(error => {
        if (!active) return

        const errorState = {
          status: "error",
          message: error instanceof Error ? error.message : String(error),
        } as const
        setState(errorState)
        onStateChange(errorState)
      })

    return () => {
      active = false
    }
  }, [address, client, onStateChange, reloadKey, updateDomains])

  if (state.status === "loading") {
    return <PoolOverviewSkeleton label="Loading nominator pool" />
  }

  if (state.status === "error") {
    return (
      <PoolLoadError
        title="Pool details are unavailable"
        message={state.message}
        onRetry={() => setReloadKey(key => key + 1)}
      />
    )
  }

  const {data} = state
  const nominatorsStake = sumNominatorStake(data)
  const totalStake = BigInt(data.validator_amount) + nominatorsStake
  const validatorShare = data.validator_reward_share / 100

  return (
    <section className={styles.card} aria-labelledby="nominator-pool-title">
      <div className={styles.header}>
        <h2 id="nominator-pool-title" className={styles.title}>
          Nominator pool
        </h2>
        <span className={`${styles.label} ${styles.validatorLabel}`}>Validator</span>
        <div className={styles.status}>
          <span className={styles[`statusDot${poolStateName(data.state)}`]} aria-hidden="true" />
          {poolStateLabel(data.state)}
        </div>
        <ExplorerAddressChip
          address={data.validator_address}
          className={styles.validatorAddress}
          onAddressClick={onAddressClick}
          variant="plain"
        />
      </div>

      <div className={styles.metrics}>
        <PoolMetric
          label="Nominators"
          value={`${data.nominators_count} of ${data.max_nominators_count}`}
        />
        <PoolMetric
          label="Total stake"
          value={<GramAmount maximumFractionDigits={2} useGrouping value={totalStake} />}
        />
        <PoolMetric
          label="Validator stake"
          value={<GramAmount maximumFractionDigits={2} useGrouping value={data.validator_amount} />}
        />
        <PoolMetric
          label="Nominators stake"
          value={<GramAmount maximumFractionDigits={2} useGrouping value={nominatorsStake} />}
        />
        <PoolMetric label="Validator income" value={`${formatPercent(validatorShare)}%`} />
        <PoolMetric label="Nominators income" value={`${formatPercent(100 - validatorShare)}%`} />
        <PoolMetric
          label="Minimum validator stake"
          value={
            <GramAmount
              maximumFractionDigits={2}
              useGrouping
              value={BigInt(data.min_validator_stake) + ONE_GRAM}
            />
          }
        />
        <PoolMetric
          label="Minimum nominator stake"
          value={
            <GramAmount
              maximumFractionDigits={2}
              useGrouping
              value={BigInt(data.min_nominator_stake) + ONE_GRAM}
            />
          }
        />
      </div>
    </section>
  )
}

export function NominatorPoolNominatorsTab({
  state,
  onAddressClick,
}: NominatorPoolNominatorsTabProps) {
  if (state.status === "loading") {
    return (
      <DataTable className={styles.flushTable} minWidth="42rem">
        <DataTableTable aria-label="Loading nominators" aria-busy="true">
          <NominatorsTableHead />
          <DataTableBody>
            <DataTableSkeletonRows columns={4} rows={3} />
          </DataTableBody>
        </DataTableTable>
      </DataTable>
    )
  }

  if (state.status === "error") {
    return <div className={styles.tabMessage}>Nominators are unavailable</div>
  }

  const totalStake = sumNominatorStake(state.data)
  const nominators = [...state.data.active_nominators].sort((left, right) =>
    compareBigIntDescending(left.balance, right.balance),
  )

  return (
    <DataTable className={styles.flushTable} minWidth="42rem">
      <DataTableTable aria-label="Nominator pool nominators">
        <NominatorsTableHead />
        <DataTableBody>
          {nominators.map(nominator => (
            <DataTableRow key={nominator.address} hover>
              <DataTableCell>
                <ExplorerAddressChip address={nominator.address} onAddressClick={onAddressClick} />
              </DataTableCell>
              <DataTableCell align="right" tone="strong">
                <GramAmount maximumFractionDigits={3} useGrouping value={nominator.balance} />
              </DataTableCell>
              <DataTableCell align="right" tone="muted">
                {formatStakeShare(nominator.balance, totalStake)}
              </DataTableCell>
              <DataTableCell align="right">
                <GramAmount
                  maximumFractionDigits={3}
                  useGrouping
                  value={nominator.pending_balance}
                />
              </DataTableCell>
            </DataTableRow>
          ))}
          {nominators.length === 0 && (
            <DataTableEmpty colSpan={4}>No active nominators</DataTableEmpty>
          )}
        </DataTableBody>
      </DataTableTable>
    </DataTable>
  )
}

export function SingleNominatorOverview({
  address,
  client,
  onAddressClick,
  version,
}: SingleNominatorOverviewProps) {
  const [state, setState] = useState<SingleNominatorLoadState>({status: "loading"})
  const [reloadKey, setReloadKey] = useState(0)

  useEffect(() => {
    let active = true
    setState({status: "loading"})

    void client
      .getSingleNominatorRoles(address)
      .then(data => {
        if (active) setState({status: "success", data})
      })
      .catch(error => {
        if (active) {
          setState({
            status: "error",
            message: error instanceof Error ? error.message : String(error),
          })
        }
      })

    return () => {
      active = false
    }
  }, [address, client, reloadKey])

  if (state.status === "loading") {
    return <SingleNominatorOverviewSkeleton />
  }

  if (state.status === "error") {
    return (
      <PoolLoadError
        title="Single nominator details are unavailable"
        message={state.message}
        onRetry={() => setReloadKey(key => key + 1)}
      />
    )
  }

  return (
    <section className={styles.card} aria-labelledby="single-nominator-title">
      <div className={styles.singleHeader}>
        <h2 id="single-nominator-title" className={styles.title}>
          Single nominator pool
        </h2>
        <span className={styles.version}>v{version}</span>
      </div>
      <div className={styles.roles}>
        <AddressRole
          label="Owner"
          address={state.data.ownerAddress}
          onAddressClick={onAddressClick}
        />
        <AddressRole
          label="Validator"
          address={state.data.validatorAddress}
          onAddressClick={onAddressClick}
        />
      </div>
    </section>
  )
}

function NominatorsTableHead() {
  return (
    <DataTableHead>
      <DataTableRow>
        <DataTableHeaderCell>Address</DataTableHeaderCell>
        <DataTableHeaderCell align="right" columnWidth="11rem">
          Stake
        </DataTableHeaderCell>
        <DataTableHeaderCell align="right" columnWidth="7rem">
          Share
        </DataTableHeaderCell>
        <DataTableHeaderCell align="right" columnWidth="11rem">
          Pending
        </DataTableHeaderCell>
      </DataTableRow>
    </DataTableHead>
  )
}

function PoolMetric({label, value}: {readonly label: string; readonly value: ReactNode}) {
  return (
    <div className={styles.metric}>
      <span className={styles.label}>{label}</span>
      <span className={styles.value}>{value}</span>
    </div>
  )
}

function AddressRole({
  address,
  label,
  onAddressClick,
}: {
  readonly address: string
  readonly label: string
  readonly onAddressClick?: (address: string, event?: MouseEvent<HTMLElement>) => void
}) {
  return (
    <div className={styles.role}>
      <span className={styles.label}>{label}</span>
      <ExplorerAddressChip address={address} onAddressClick={onAddressClick} variant="plain" />
    </div>
  )
}

function PoolLoadError({
  message,
  onRetry,
  title,
}: {
  readonly message: string
  readonly onRetry: () => void
  readonly title: string
}) {
  return (
    <section className={styles.card} aria-label={title}>
      <div className={styles.error}>
        <div>
          <div className={styles.errorTitle}>{title}</div>
          <div className={styles.errorMessage}>{message}</div>
        </div>
        <Button type="button" size="sm" variant="secondary" onClick={onRetry}>
          Retry
        </Button>
      </div>
    </section>
  )
}

function PoolOverviewSkeleton({label}: {readonly label: string}) {
  return (
    <section className={styles.card} aria-label={label} aria-busy="true">
      <div className={styles.header}>
        <Skeleton width="9rem" />
        <Skeleton width="4rem" />
        <Skeleton width="4rem" />
        <Skeleton width="9rem" />
      </div>
      <div className={styles.metrics}>
        {Array.from({length: 8}, (_, index) => (
          <div className={styles.metric} key={index}>
            <Skeleton width="65%" />
            <Skeleton width="80%" />
          </div>
        ))}
      </div>
    </section>
  )
}

function SingleNominatorOverviewSkeleton() {
  return (
    <section className={styles.card} aria-label="Loading single nominator pool" aria-busy="true">
      <div className={styles.singleHeader}>
        <Skeleton width="10rem" />
        <Skeleton width="3rem" />
      </div>
      <div className={styles.roles}>
        {Array.from({length: 2}, (_, index) => (
          <div className={styles.role} key={index}>
            <Skeleton width="4rem" />
            <Skeleton width="10rem" />
          </div>
        ))}
      </div>
    </section>
  )
}

function sumNominatorStake(pool: V3NominatorPool): bigint {
  return pool.active_nominators.reduce((sum, nominator) => sum + BigInt(nominator.balance), 0n)
}

function compareBigIntDescending(left: string, right: string): number {
  const leftValue = BigInt(left)
  const rightValue = BigInt(right)
  return leftValue === rightValue ? 0 : leftValue > rightValue ? -1 : 1
}

function formatStakeShare(value: string, total: bigint): string {
  if (total === 0n) return "—"

  const thousandths = (BigInt(value) * 100_000n) / total
  return `${formatPercent(Number(thousandths) / 1000)}%`
}

function formatPercent(value: number): string {
  return value.toLocaleString("en-US", {maximumFractionDigits: 3})
}

function poolStateLabel(state: number): string {
  if (state === 2) return "Active"
  if (state === 1) return "Pending"
  return "Inactive"
}

function poolStateName(state: number): "Active" | "Inactive" | "Pending" {
  if (state === 2) return "Active"
  if (state === 1) return "Pending"
  return "Inactive"
}
