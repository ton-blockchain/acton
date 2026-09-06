import {useCallback, useEffect, useLayoutEffect, useRef, useState} from "react"
import type {ReactNode} from "react"
import {ArrowRightLeft, Check, Coins, Image, Layers, Play, Square} from "lucide-react"
import {
  Button,
  Checkbox,
  DataTable,
  DataTableBody,
  DataTableCell,
  DataTableHead,
  DataTableHeaderCell,
  DataTableRow,
  DataTableTable,
  Disclosure,
  Input,
  Skeleton,
  Slider,
  useToast,
} from "@acton/ui"
import {Metric} from "@acton/localton-ui"
import {ExplorerAddressChip} from "@acton/explorer-core/components/ExplorerAddressChip"
import {useExplorerRoutePaths} from "@acton/explorer-core/hooks/useExplorerRoutePaths"
import {useOpenExplorerPath} from "@acton/explorer-core/hooks/useOpenExplorerPath"

import {controlNetworkActivity, fetchNetworkActivity} from "../../../networkActivityApi"
import type {
  ActivityConfig,
  ActivityRun,
  ActivityScenario,
  ActivityState,
  ActivityWalletVersion,
} from "../../../networkActivityApi"
import type {StudioEnvironment} from "../../../studioApi"
import styles from "./ActivityPage.module.css"

const scenarios = [
  {
    id: "transfers",
    name: "Transfers",
    description: "Send GRAM between fresh wallets",
    icon: ArrowRightLeft,
  },
  {
    id: "batches",
    name: "Batch transfers",
    description: "Send to multiple recipients with a V5 wallet",
    icon: Layers,
  },
  {
    id: "jettons",
    name: "Jettons",
    description: "Deploy a token, mint, transfer and burn",
    icon: Coins,
  },
  {
    id: "nfts",
    name: "NFTs",
    description: "Create a collection, mint an item and transfer it",
    icon: Image,
  },
] as const

type Draft = {
  intervalSeconds: string
  scenariosPerLaunch: string
  concurrency: string
  durationSeconds: string
  maxBatchSize: string
  randomizeBatchSize: boolean
  transferAmount: string
  walletVersions: readonly ActivityWalletVersion[]
  scenarios: Record<ActivityScenario, string>
}

interface ActivityPageProps {
  readonly environment: StudioEnvironment
  readonly onActionsChange: (actions: ReactNode) => void
}

/** The page edits a workload; polling only observes work owned by the localnet service */
export function ActivityPage({environment, onActionsChange}: ActivityPageProps) {
  const {showToast, updateToast} = useToast()
  const routes = useExplorerRoutePaths()
  const openExplorerPath = useOpenExplorerPath()
  const [state, setState] = useState<ActivityState>()
  const [draft, setDraft] = useState<Draft>()
  const [pending, setPending] = useState<"save" | "start" | "stop">()
  const [available, setAvailable] = useState(true)
  const [retry, setRetry] = useState(0)
  const latest = useRef<ActivityState | undefined>(undefined)
  const revision = useRef(0)
  const mutating = useRef(false)
  const mounted = useRef(true)
  const running = state?.status === "running" || state?.status === "stopping"
  const editable = !running && !pending
  const dirty = state && draft && JSON.stringify(draft) !== JSON.stringify(toDraft(state.config))

  useEffect(() => {
    mounted.current = true
    let timer: ReturnType<typeof setTimeout> | undefined
    const controller = new AbortController()
    let reportedError = false

    const poll = async () => {
      try {
        if (!mutating.current) {
          const version = revision.current
          try {
            const next = await fetchNetworkActivity(environment.id, controller.signal)
            if (controller.signal.aborted || version !== revision.current) return
            const previous = latest.current
            if (previous?.runId === next.runId) {
              if (next.failed > previous.failed) {
                const failure = next.recent.find(run => run.outcome === "failed")
                showToast({
                  variant: "error",
                  title: "Activity scenario failed",
                  description: failure?.error ?? "Inspect the localnet logs for details",
                })
              }
              if (previous.status === "running" && next.status === "completed") {
                showToast({
                  variant: "success",
                  title: "Activity finished",
                  description: `${next.completed} scenarios completed in ${environment.name}`,
                })
              }
              if (previous.status === "running" && next.status === "interrupted") {
                showToast({
                  variant: "error",
                  title: "Activity interrupted",
                  description: "The localnet service stopped before this run finished",
                })
              }
            }
            latest.current = next
            setState(next)
            setDraft(current => current ?? toDraft(next.config))
            setAvailable(true)
            reportedError = false
          } catch (error) {
            if (controller.signal.aborted) return
            setAvailable(false)
            if (!reportedError) {
              showToast({
                variant: "error",
                title: "Activity unavailable",
                description: errorMessage(error),
              })
              reportedError = true
            }
          }
        }
      } finally {
        if (!controller.signal.aborted) timer = setTimeout(poll, 2000)
      }
    }
    void poll()

    return () => {
      mounted.current = false
      controller.abort()
      clearTimeout(timer)
    }
  }, [environment.id, environment.name, retry, showToast])

  const control = useCallback(
    async (command: "save" | "start" | "stop") => {
      if (mutating.current || !draft) return
      let config: ActivityConfig | undefined
      try {
        if (command !== "stop") config = fromDraft(draft)
      } catch (error) {
        showToast({
          variant: "error",
          title: "Check activity settings",
          description: errorMessage(error),
        })
        return
      }

      mutating.current = true
      revision.current += 1
      setPending(command)
      const toast = showToast({
        variant: "loading",
        title:
          command === "start"
            ? "Starting activity"
            : command === "stop"
              ? "Stopping activity"
              : "Saving activity settings",
        description:
          command === "stop"
            ? "Waiting for in-flight wallet funding requests to finish"
            : undefined,
        durationMs: 0,
      })
      try {
        const next = await controlNetworkActivity(environment.id, command, config)
        revision.current += 1
        if (mounted.current) {
          latest.current = next
          setState(next)
          setDraft(toDraft(next.config))
          setAvailable(true)
        }
        updateToast(toast, {
          variant: "success",
          title:
            command === "start"
              ? "Activity started"
              : command === "stop"
                ? "Activity stopped"
                : "Activity settings saved",
          description: environment.name,
          durationMs: 5000,
        })
      } catch (error) {
        updateToast(toast, {
          variant: "error",
          title:
            command === "start"
              ? "Activity not started"
              : command === "stop"
                ? "Activity not stopped"
                : "Settings not saved",
          description: errorMessage(error),
          durationMs: 8000,
        })
      } finally {
        mutating.current = false
        if (mounted.current) setPending(undefined)
      }
    },
    [draft, environment.id, environment.name, showToast, updateToast],
  )

  useLayoutEffect(() => {
    onActionsChange(
      <div className={styles.actions}>
        {!running && dirty ? (
          <Button
            variant="outline"
            loading={pending === "save"}
            disabled={!!pending}
            onClick={() => void control("save")}
          >
            Save settings
          </Button>
        ) : null}
        <Button
          variant={running ? "outline" : "primary"}
          leadingIcon={running ? <Square size={15} /> : <Play size={15} />}
          loading={pending === "start" || pending === "stop"}
          disabled={
            !draft || !!pending || (!running && (environment.status !== "running" || !available))
          }
          title={
            !running && environment.status !== "running"
              ? "Start the network to generate activity"
              : undefined
          }
          onClick={() => void control(running ? "stop" : "start")}
        >
          {pending === "stop"
            ? "Stopping"
            : pending === "start"
              ? "Starting"
              : running
                ? "Stop activity"
                : "Start activity"}
        </Button>
      </div>,
    )
    return () => onActionsChange(undefined)
  }, [available, control, dirty, draft, environment.status, onActionsChange, pending, running])

  if (!draft || !state) {
    return available ? (
      <ActivityPageSkeleton />
    ) : (
      <Button variant="outline" onClick={() => setRetry(value => value + 1)}>
        Retry
      </Button>
    )
  }

  const weights = scenarios.map(({id}) => Math.max(0, Number(draft.scenarios[id]) || 0))
  const total = weights.reduce((sum, weight) => sum + weight, 0)
  const setField = (
    field: keyof Omit<Draft, "scenarios" | "walletVersions" | "randomizeBatchSize">,
    value: string,
  ) => setDraft({...draft, [field]: value})
  const showFailure = (run: ActivityRun) =>
    showToast({
      variant: "error",
      title: "Activity scenario failed",
      description: run.error ?? "Inspect the localnet logs for details",
    })

  return (
    <div className={styles.page}>
      <section className={styles.metrics} aria-label="Current run">
        <Metric label="Confirmed messages" value={state.confirmedMessages.toLocaleString()} />
        <Metric label="Completed scenarios" value={state.completed.toLocaleString()} />
        <Metric label="Active scenarios" value={`${state.active} / ${state.config.concurrency}`} />
        <Metric label="Failed scenarios" value={state.failed.toLocaleString()} />
      </section>

      <div className={styles.layout}>
        <section className={styles.panel} aria-labelledby="activity-scenarios">
          <div className={styles.panelHeading}>
            <h2 id="activity-scenarios">Activity mix</h2>
            <span className={styles.hint}>Choose scenarios and their relative frequency</span>
          </div>
          <div className={styles.mix} aria-label="Scenario frequency">
            {scenarios.map(({id, name}, index) =>
              weights[index] > 0 ? (
                <span
                  key={id}
                  className={styles[id]}
                  style={{flex: weights[index]}}
                  title={`${name}: ${Math.round((weights[index] / total) * 100)}%`}
                />
              ) : null,
            )}
          </div>
          <div className={styles.scenarios}>
            {scenarios.map(({id, name, description, icon: Icon}, index) => (
              <div
                key={id}
                className={`${styles.scenario} ${weights[index] ? "" : styles.disabledScenario}`}
              >
                <Icon className={styles[id]} size={20} aria-hidden="true" />
                <div className={styles.scenarioText}>
                  <strong>{name}</strong>
                  <span>{description}</span>
                </div>
                <div className={styles.weight}>
                  <Slider
                    aria-label={`${name} weight`}
                    aria-valuetext={`${total ? Math.round((weights[index] / total) * 100) : 0}% of scenario starts`}
                    className={styles[id]}
                    value={weights[index]}
                    disabled={!editable}
                    onValueChange={value =>
                      setDraft({...draft, scenarios: {...draft.scenarios, [id]: String(value)}})
                    }
                  />
                  <span>{total ? Math.round((weights[index] / total) * 100) : 0}%</span>
                </div>
              </div>
            ))}
          </div>
          <Disclosure label="Scenario settings" contentClassName={styles.advanced}>
            <div className={styles.fields}>
              <Input
                label="Transfer amount"
                suffix="GRAM"
                type="number"
                min="0.001"
                max="1000"
                step="0.001"
                value={draft.transferAmount}
                disabled={!editable}
                onChange={event => setField("transferAmount", event.target.value)}
              />
              <div className={styles.batchSettings}>
                <Input
                  label="Maximum batch size"
                  type="number"
                  min={2}
                  max={128}
                  value={draft.maxBatchSize}
                  disabled={!editable}
                  onChange={event => setField("maxBatchSize", event.target.value)}
                />
                <Checkbox
                  label="Randomize batch size"
                  checked={draft.randomizeBatchSize}
                  disabled={!editable}
                  title="Choose a new size from 2 to the maximum for each batch scenario"
                  onChange={event =>
                    setDraft({...draft, randomizeBatchSize: event.target.checked})
                  }
                />
              </div>
            </div>
            <fieldset className={styles.wallets} disabled={!editable}>
              <legend>Wallet versions</legend>
              {(["v3r2", "v4r2", "v5r1"] as const).map(version => (
                <Checkbox
                  key={version}
                  label={version}
                  checked={draft.walletVersions.includes(version)}
                  onChange={event =>
                    setDraft({
                      ...draft,
                      walletVersions: event.target.checked
                        ? [...draft.walletVersions, version]
                        : draft.walletVersions.filter(item => item !== version),
                    })
                  }
                />
              ))}
            </fieldset>
            <p className={styles.hint}>Batch transfers always use v5r1</p>
          </Disclosure>
        </section>

        <section className={styles.panel} aria-labelledby="activity-load">
          <div className={styles.panelHeading}>
            <h2 id="activity-load">Workload</h2>
          </div>
          <div className={styles.presets} role="group" aria-label="Workload presets">
            {(
              [
                {label: "Quiet", interval: "30", count: "1", concurrency: "2"},
                {label: "Steady", interval: "5", count: "5", concurrency: "32"},
                {label: "Busy", interval: "1", count: "20", concurrency: "256"},
              ] as const
            ).map(preset => (
              <Button
                key={preset.label}
                size="sm"
                variant="secondary"
                aria-pressed={
                  draft.intervalSeconds === preset.interval &&
                  draft.scenariosPerLaunch === preset.count &&
                  draft.concurrency === preset.concurrency
                }
                disabled={!editable}
                onClick={() =>
                  setDraft({
                    ...draft,
                    intervalSeconds: preset.interval,
                    scenariosPerLaunch: preset.count,
                    concurrency: preset.concurrency,
                  })
                }
              >
                {preset.label}
              </Button>
            ))}
          </div>
          <div className={styles.workload}>
            <Input
              label="Launch interval"
              suffix="s"
              type="number"
              min={1}
              max={3600}
              value={draft.intervalSeconds}
              disabled={!editable}
              onChange={event => setField("intervalSeconds", event.target.value)}
            />
            <Input
              label="Scenarios per launch"
              type="number"
              min={1}
              max={1000}
              value={draft.scenariosPerLaunch}
              disabled={!editable}
              onChange={event => setField("scenariosPerLaunch", event.target.value)}
            />
            <Input
              label="Concurrent scenarios"
              description="Starts above this limit are skipped"
              type="number"
              min={1}
              max={1024}
              value={draft.concurrency}
              disabled={!editable}
              onChange={event => setField("concurrency", event.target.value)}
            />
            <Input
              label="Run duration"
              suffix="s"
              description="0 to run until stopped"
              type="number"
              min={0}
              max={86400}
              value={draft.durationSeconds}
              disabled={!editable}
              onChange={event => setField("durationSeconds", event.target.value)}
            />
          </div>
        </section>
      </div>

      <section className={styles.history} aria-labelledby="activity-history">
        <div className={styles.historyHeading}>
          <h2 id="activity-history">Recent scenarios</h2>
          <span className={styles.hint}>
            {state.skipped
              ? `${state.skipped} starts skipped while all slots were busy`
              : "Messages are counted after confirmation"}
          </span>
        </div>
        {state.recent.length ? (
          <DataTable>
            <DataTableTable>
              <DataTableHead>
                <DataTableRow>
                  <DataTableHeaderCell>Scenario</DataTableHeaderCell>
                  <DataTableHeaderCell>Wallet</DataTableHeaderCell>
                  <DataTableHeaderCell>Messages</DataTableHeaderCell>
                  <DataTableHeaderCell>Duration</DataTableHeaderCell>
                  <DataTableHeaderCell>Result</DataTableHeaderCell>
                </DataTableRow>
              </DataTableHead>
              <DataTableBody>
                {state.recent.map(run => (
                  <DataTableRow key={run.id}>
                    <DataTableCell>
                      {scenarios.find(scenario => scenario.id === run.scenario)?.name}
                    </DataTableCell>
                    <DataTableCell>
                      {run.address ? (
                        <ExplorerAddressChip
                          address={run.address}
                          onAddressClick={(address, event) =>
                            openExplorerPath(routes.addressPath(address), event)
                          }
                        />
                      ) : (
                        "—"
                      )}
                    </DataTableCell>
                    <DataTableCell>{run.confirmedMessages}</DataTableCell>
                    <DataTableCell>{(run.durationMs / 1000).toFixed(1)} s</DataTableCell>
                    <DataTableCell>
                      {run.outcome === "failed" ? (
                        <button
                          type="button"
                          className={styles.failure}
                          onClick={() => showFailure(run)}
                        >
                          View error
                        </button>
                      ) : (
                        <span className={styles.outcome}>
                          {run.outcome === "completed" ? (
                            <Check size={14} aria-hidden="true" />
                          ) : null}
                          {run.outcome === "completed" ? "Completed" : "Cancelled"}
                        </span>
                      )}
                    </DataTableCell>
                  </DataTableRow>
                ))}
              </DataTableBody>
            </DataTableTable>
          </DataTable>
        ) : (
          <div className={styles.empty}>
            <ArrowRightLeft size={22} aria-hidden="true" />
            <span>Your generated activity will appear here</span>
          </div>
        )}
      </section>
    </div>
  )
}

function ActivityPageSkeleton() {
  return (
    <div className={styles.page} aria-busy="true" aria-label="Loading activity settings">
      <div className={styles.metrics}>
        {["Confirmed messages", "Completed scenarios", "Active scenarios", "Failed scenarios"].map(
          label => (
            <Metric key={label} label={label} value={<Skeleton width={56} height={26} />} />
          ),
        )}
      </div>
      <div className={styles.layout}>
        <div className={styles.panel}>
          <div className={styles.panelHeading}>
            <h2>Activity mix</h2>
            <span className={styles.hint}>Choose scenarios and their relative frequency</span>
          </div>
          <Skeleton width="100%" height={6} />
          <div className={styles.scenarios}>
            {scenarios.map(({id, name, description, icon: Icon}) => (
              <div key={id} className={styles.scenario}>
                <Icon size={20} className={styles.skeletonIcon} aria-hidden="true" />
                <div className={styles.scenarioText}>
                  <strong>{name}</strong>
                  <span>{description}</span>
                </div>
                <div className={styles.weight}>
                  <Skeleton width="100%" height={4} />
                  <Skeleton width={28} height={12} />
                </div>
              </div>
            ))}
          </div>
          <Disclosure label="Scenario settings">
            <Skeleton height={36} />
          </Disclosure>
        </div>
        <div className={styles.panel}>
          <div className={styles.panelHeading}>
            <h2>Workload</h2>
          </div>
          <div className={styles.presets} role="group" aria-label="Workload presets">
            {["Quiet", "Steady", "Busy"].map(label => (
              <Button key={label} size="sm" variant="secondary" disabled>
                {label}
              </Button>
            ))}
          </div>
          <div className={styles.workload}>
            <Input label="Launch interval" suffix="s" disabled />
            <Input label="Scenarios per launch" disabled />
            <Input
              label="Concurrent scenarios"
              description="Starts above this limit are skipped"
              disabled
            />
            <Input label="Run duration" suffix="s" description="0 to run until stopped" disabled />
          </div>
        </div>
      </div>
      <div className={styles.history}>
        <div className={styles.historyHeading}>
          <h2>Recent scenarios</h2>
        </div>
        <div className={styles.empty}>
          <Skeleton width={230} height={14} />
        </div>
      </div>
    </div>
  )
}

function toDraft(config: ActivityConfig): Draft {
  return {
    intervalSeconds: String(config.intervalSeconds),
    scenariosPerLaunch: String(config.scenariosPerLaunch),
    concurrency: String(config.concurrency),
    durationSeconds: String(config.durationSeconds),
    maxBatchSize: String(config.maxBatchSize),
    randomizeBatchSize: config.randomizeBatchSize,
    transferAmount: String(config.transferAmount / 1e9),
    walletVersions: config.walletVersions,
    scenarios: {
      transfers: String(config.scenarios.transfers),
      batches: String(config.scenarios.batches),
      jettons: String(config.scenarios.jettons),
      nfts: String(config.scenarios.nfts),
    },
  }
}

function fromDraft(draft: Draft): ActivityConfig {
  const integer = (value: string, label: string, min: number, max: number) => {
    const number = Number(value)
    if (!value.trim() || !Number.isInteger(number) || number < min || number > max)
      throw new Error(`${label} must be a whole number between ${min} and ${max}`)
    return number
  }
  const transferAmount = Math.round(Number(draft.transferAmount) * 1e9)
  if (!Number.isSafeInteger(transferAmount) || transferAmount < 1e6 || transferAmount > 1e12)
    throw new Error("Enter a transfer amount between 0.001 and 1000 GRAM")
  const weights = Object.fromEntries(
    scenarios.map(({id, name}) => [id, integer(draft.scenarios[id], `${name} weight`, 0, 100)]),
  ) as Record<ActivityScenario, number>
  if (!Object.values(weights).some(Boolean))
    throw new Error("Enable at least one activity scenario")
  if (!draft.walletVersions.length) throw new Error("Select at least one wallet version")
  return {
    intervalSeconds: integer(draft.intervalSeconds, "Scenario interval", 1, 3600),
    scenariosPerLaunch: integer(draft.scenariosPerLaunch, "Scenarios per launch", 1, 1000),
    concurrency: integer(draft.concurrency, "Concurrent scenarios", 1, 1024),
    durationSeconds: integer(draft.durationSeconds, "Run duration", 0, 86400),
    maxBatchSize: integer(draft.maxBatchSize, "Maximum batch size", 2, 128),
    randomizeBatchSize: draft.randomizeBatchSize,
    transferAmount,
    walletVersions: draft.walletVersions,
    scenarios: weights,
  }
}

function errorMessage(error: unknown) {
  return error instanceof Error ? error.message : String(error)
}
