import {requestJson} from "./studioApi"

export type ActivityScenario = "transfers" | "batches" | "jettons" | "nfts"
export type ActivityWalletVersion = "v3r2" | "v4r2" | "v5r1"

export interface ActivityConfig {
  readonly intervalSeconds: number
  readonly scenariosPerLaunch: number
  readonly concurrency: number
  readonly durationSeconds: number
  readonly maxBatchSize: number
  readonly randomizeBatchSize: boolean
  readonly transferAmount: number
  readonly walletVersions: readonly ActivityWalletVersion[]
  readonly scenarios: Readonly<Record<ActivityScenario, number>>
}

export interface ActivityRun {
  readonly id: number
  readonly scenario: ActivityScenario
  readonly startedAt: number
  readonly durationMs: number
  readonly batchSize: number | null
  readonly address: string | null
  readonly confirmedMessages: number
  readonly outcome: "completed" | "failed" | "cancelled"
  readonly error: string | null
}

export interface ActivityState {
  readonly config: ActivityConfig
  readonly status: "stopped" | "running" | "stopping" | "completed" | "interrupted"
  readonly runId: string | null
  readonly startedAt: number | null
  readonly finishedAt: number | null
  readonly active: number
  readonly completed: number
  readonly failed: number
  readonly confirmedMessages: number
  readonly skipped: number
  readonly recent: readonly ActivityRun[]
}

export function fetchNetworkActivity(environmentId: string, signal: AbortSignal) {
  return requestJson<ActivityState>(activityPath(environmentId), {signal})
}

export function controlNetworkActivity(
  environmentId: string,
  command: "save" | "start" | "stop",
  config?: ActivityConfig,
) {
  return requestJson<ActivityState>(
    `${activityPath(environmentId)}${command === "save" ? "" : `/${command}`}`,
    {
      method: command === "save" ? "PUT" : "POST",
      headers: {"content-type": "application/json"},
      body: config ? JSON.stringify(config) : undefined,
    },
  )
}

function activityPath(environmentId: string) {
  return `/api/v1/environments/${encodeURIComponent(environmentId)}/network/activity`
}
