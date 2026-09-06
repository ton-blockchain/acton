import {useEffect, useRef, useState} from "react"
import {Clock3} from "lucide-react"
import {
  BooleanValue,
  DataTable,
  DataTableBody,
  DataTableCell,
  DataTableEmpty,
  DataTableHead,
  DataTableHeaderCell,
  DataTableRow,
  DataTableTable,
  Disclosure,
  Percentage,
  TechnicalValue,
  Tooltip,
} from "@acton/ui"

import type {ElectionObservation, ValidatorObservation, ValidatorSetObservation} from "../types"
import {Metric} from "./Metric"
import styles from "./ElectionSection.module.css"

interface ElectionSectionProps {
  readonly election: ElectionObservation | null
  readonly now: number
}

const VALIDATOR_PREVIEW_COUNT = 7

/** Owns the election timeline and validator-set presentation for an aggregated network view */
export function ElectionSection({election, now}: ElectionSectionProps) {
  return (
    <section id="elections" className={styles.sectionStack} aria-label="Validator elections">
      {election ? (
        <ElectionDiagram election={election} now={now} />
      ) : (
        <div className={styles.notice}>
          <Clock3 size={16} aria-hidden="true" />
          <span>Election data is not available from the current chain view</span>
        </div>
      )}
    </section>
  )
}

function ElectionDiagram({
  election,
  now,
}: {
  readonly election: ElectionObservation
  readonly now: number
}) {
  const previousCurrentRoundId = useRef(election.current.round_id)
  const rollingOver = previousCurrentRoundId.current !== election.current.round_id

  useEffect(() => {
    previousCurrentRoundId.current = election.current.round_id
  }, [election.current.round_id])

  const duration = Math.max(1, election.validators_elected_for)
  const previous =
    election.previous ??
    inferredValidatorSet(election.current.validation_started_at - duration, duration)
  const next = election.next ?? inferredValidatorSet(election.current.validation_ended_at, duration)
  const rounds = [
    {
      kind: "previous",
      label: "Previous round",
      set: previous,
      available: election.previous !== null,
      unavailableLabel: "Set unavailable",
    },
    {
      kind: "current",
      label: "Current round",
      set: election.current,
      available: true,
      unavailableLabel: "Set unavailable",
    },
    {
      kind: "next",
      label: "Next round",
      set: next,
      available: election.next !== null,
      unavailableLabel: "Pending",
    },
  ]
  const entryStart = (set: ValidatorSetObservation) =>
    set.validation_started_at - (election.current.validation_ended_at - election.elections_open_at)
  const entryEnd = (set: ValidatorSetObservation) =>
    set.validation_started_at - (election.current.validation_ended_at - election.elections_close_at)
  const rangeStart = Math.min(...rounds.map(round => entryStart(round.set)), now)
  const rangeEnd = Math.max(
    ...rounds.map(round => round.set.validation_ended_at + election.stake_held_for),
    now + Math.floor(duration / 4),
  )
  const range = Math.max(1, rangeEnd - rangeStart)
  const position = (value: number) =>
    Math.min(100, Math.max(0, ((value - rangeStart) / range) * 100))
  const width = (start: number, end: number) => Math.max(0.4, position(end) - position(start))
  const nowPosition = position(now)

  return (
    <div className={styles.electionPanel}>
      <div className={styles.electionSummary}>
        <Metric
          density="compact"
          label="Round ID"
          value={election.current.round_id.toLocaleString()}
        />
        <Metric
          density="compact"
          label="Current set"
          value={formatValidators(election.current.validators)}
        />
        <Metric
          density="compact"
          label="Main subset"
          value={formatValidators(election.current.main_validators)}
        />
        <Metric
          density="compact"
          label="Next set"
          value={election.next === null ? "Pending" : formatValidators(election.next.validators)}
        />
        <Metric density="compact" label="Stake hold" value={`${election.stake_held_for}s`} />
      </div>
      <div className={styles.electionChart} data-stage={election.stage}>
        <div
          aria-label="Validator election timeline"
          className={styles.electionTimeline}
          data-rollover={rollingOver}
          role="img"
        >
          <div className={styles.timelineNow} style={{left: `${nowPosition}%`}}>
            <strong>NOW</strong>
          </div>
          {rounds.map(round => {
            const openedAt = entryStart(round.set)
            const closedAt = entryEnd(round.set)
            const validationEndedAt = round.set.validation_ended_at
            const holdingEndedAt = validationEndedAt + election.stake_held_for
            const phases = [
              {name: "Election", className: styles.timelineEntry, start: openedAt, end: closedAt},
              {
                name: "Selection",
                className: styles.timelineSelection,
                start: closedAt,
                end: round.set.validation_started_at,
              },
              {
                name: "Validation",
                className: styles.timelineValidation,
                start: round.set.validation_started_at,
                end: validationEndedAt,
              },
              {
                name: "Stake hold",
                className: styles.timelineHolding,
                start: validationEndedAt,
                end: holdingEndedAt,
              },
            ]
            const activePhase = phases.find(phase => phase.start <= now && now < phase.end)

            return (
              <div
                className={styles.electionRound}
                data-active={activePhase !== undefined}
                data-current={round.kind === "current"}
                key={round.set.round_id}
              >
                <div className={styles.electionRoundHeading}>
                  <strong>{round.label}</strong>
                  <span>#{round.set.round_id.toLocaleString()}</span>
                  <span>
                    {round.available
                      ? formatValidators(round.set.validators)
                      : round.unavailableLabel}
                  </span>
                </div>
                <div className={styles.electionRoundTrack}>
                  {phases.map(phase => {
                    const tooltip = `${phase.name} · ${formatTimestamp(phase.start)}–${formatTimestamp(phase.end)}`

                    return (
                      <Tooltip content={tooltip} delay={0} key={phase.name}>
                        <span
                          aria-current={phase === activePhase ? "true" : undefined}
                          aria-label={tooltip}
                          className={`${styles.timelineSegment} ${phase.className}`}
                          data-active={phase === activePhase}
                          style={{
                            left: `${position(phase.start)}%`,
                            width: `${width(phase.start, phase.end)}%`,
                          }}
                        />
                      </Tooltip>
                    )
                  })}
                  {activePhase ? (
                    <span
                      className={styles.timelineNowDot}
                      style={{left: `${nowPosition}%`}}
                      aria-hidden="true"
                    />
                  ) : null}
                </div>
                <div className={styles.electionRoundPhases} aria-hidden="true">
                  <span style={{left: `${position((openedAt + closedAt) / 2)}%`}}>Election</span>
                  <span
                    style={{
                      left: `${position((closedAt + round.set.validation_started_at) / 2)}%`,
                    }}
                  >
                    Selection
                  </span>
                  <span
                    style={{
                      left: `${position((round.set.validation_started_at + validationEndedAt) / 2)}%`,
                    }}
                  >
                    Validation
                  </span>
                  <span style={{left: `${position((validationEndedAt + holdingEndedAt) / 2)}%`}}>
                    Holding
                  </span>
                </div>
              </div>
            )
          })}
        </div>
      </div>
      <ValidatorSetTables election={election} />
    </div>
  )
}

function ValidatorSetTables({election}: {readonly election: ElectionObservation}) {
  const sets = [
    {label: "Previous set", set: election.previous, unavailableLabel: "Set unavailable"},
    {label: "Current set", set: election.current, unavailableLabel: "Set unavailable"},
    {label: "Next set", set: election.next, unavailableLabel: "Pending"},
  ]

  return (
    <div className={styles.validatorSetDisclosures}>
      {sets.map(({label, set, unavailableLabel}) => (
        <Disclosure
          className={styles.validatorSetDisclosure}
          contentClassName={styles.validatorSetContent}
          key={label}
          label={
            <span className={styles.validatorSetSummary}>
              <span>{label}</span>
              <span className={styles.validatorSetSummaryMeta}>
                {set
                  ? `${formatValidators(set.validators)} · ${formatTimestamp(set.validation_started_at)}–${formatTimestamp(set.validation_ended_at)}`
                  : unavailableLabel}
              </span>
            </span>
          }
        >
          {set ? (
            <ValidatorSetTable label={label} set={set} />
          ) : (
            <div className={styles.validatorSetUnavailable}>{unavailableLabel}</div>
          )}
        </Disclosure>
      ))}
    </div>
  )
}

function ValidatorSetTable({
  label,
  set,
}: {
  readonly label: string
  readonly set: ValidatorSetObservation
}) {
  const [expanded, setExpanded] = useState(false)
  const validators = set.members ?? []
  const hasMore = validators.length > VALIDATOR_PREVIEW_COUNT
  const visibleValidators = expanded
    ? validators
    : validators.slice(0, VALIDATOR_PREVIEW_COUNT + (hasMore ? 1 : 0))

  return (
    <DataTable
      minWidth="48rem"
      preview={
        hasMore
          ? {
              expanded,
              itemLabel: "validators",
              onExpandedChange: setExpanded,
            }
          : undefined
      }
      variant="embedded"
    >
      <DataTableTable aria-label={`${label} validators`}>
        <DataTableHead>
          <DataTableRow>
            <DataTableHeaderCell columnWidth="3.5rem">#</DataTableHeaderCell>
            <DataTableHeaderCell>Public key</DataTableHeaderCell>
            <DataTableHeaderCell>ADNL</DataTableHeaderCell>
            <DataTableHeaderCell columnWidth="8rem">Masterchain</DataTableHeaderCell>
            <DataTableHeaderCell columnWidth="10rem">Weight share</DataTableHeaderCell>
          </DataTableRow>
        </DataTableHead>
        <DataTableBody>
          {set.members === undefined ? (
            <DataTableEmpty colSpan={5}>Validator identities are not available</DataTableEmpty>
          ) : visibleValidators.length === 0 ? (
            <DataTableEmpty colSpan={5}>No validators in this set</DataTableEmpty>
          ) : (
            visibleValidators.map((validator, index) => (
              <ValidatorSetRow
                index={index}
                key={`${validator.public_key}:${index}`}
                mainValidatorCount={set.main_validators}
                totalWeight={set.total_weight ?? "0"}
                validator={validator}
              />
            ))
          )}
        </DataTableBody>
      </DataTableTable>
    </DataTable>
  )
}

function ValidatorSetRow({
  index,
  mainValidatorCount,
  totalWeight,
  validator,
}: {
  readonly index: number
  readonly mainValidatorCount: number
  readonly totalWeight: string
  readonly validator: ValidatorObservation
}) {
  const weightParts = validatorWeightParts(validator.weight, totalWeight)

  return (
    <DataTableRow hover>
      <DataTableCell tone="muted">{index + 1}</DataTableCell>
      <DataTableCell truncate>
        <TechnicalValue
          copyLabel="validator public key"
          endLength={10}
          startLength={10}
          value={validator.public_key}
        />
      </DataTableCell>
      <DataTableCell truncate>
        <TechnicalValue
          copyLabel="validator ADNL address"
          endLength={10}
          fallback="—"
          startLength={10}
          value={validator.adnl_address}
        />
      </DataTableCell>
      <DataTableCell>
        <BooleanValue value={index < mainValidatorCount} />
      </DataTableCell>
      <DataTableCell>
        <Tooltip
          content={`${BigInt(validator.weight).toLocaleString()} of ${BigInt(totalWeight).toLocaleString()}`}
        >
          <span className={styles.validatorWeightShare}>
            <Percentage
              maximumFractionDigits={3}
              minimumFractionDigits={2}
              total={1_000_000}
              value={weightParts}
            />
          </span>
        </Tooltip>
      </DataTableCell>
    </DataTableRow>
  )
}

/** Preserves large TON weights while scaling their ratio for Percentage */
export function validatorWeightParts(weight: string, totalWeight: string) {
  const total = BigInt(totalWeight)
  if (total === 0n) return 0

  return Number((BigInt(weight) * 1_000_000n) / total)
}

function inferredValidatorSet(start: number, duration: number): ValidatorSetObservation {
  return {
    round_id: start,
    validation_started_at: start,
    validation_ended_at: start + duration,
    validators: 0,
    main_validators: 0,
    total_weight: "0",
    members: [],
  }
}

function formatTimestamp(value: number) {
  return new Intl.DateTimeFormat(undefined, {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  }).format(value * 1000)
}

function formatValidators(count: number) {
  return `${count.toLocaleString()} ${count === 1 ? "validator" : "validators"}`
}
