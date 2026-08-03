import type {TestReport} from "@acton/test-ui/embed"

export const STUDIO_API_VERSION = 1

export interface StudioInfo {
  readonly protocolVersion: number
  readonly serverVersion: string
  readonly workspace?: {
    readonly name: string
    readonly walletNames?: readonly string[]
  }
}

export type StudioConnectionState = "connecting" | "connected" | "disconnected"

export type EnvironmentStatus = "starting" | "running" | "stopping" | "stopped" | "failed"
export type EnvironmentLifecycle = "managed" | "external"

export interface ActonLocalnetEnvironmentConfig {
  readonly kind: "actonLocalnet"
  readonly port: number
  readonly forkNetwork?: string
  readonly forkBlockNumber?: number
  readonly accounts: readonly string[]
  readonly rateLimit?: number
  readonly responseDelayMs?: number
  readonly blockIntervalMs?: number
  readonly noMining: boolean
  readonly mineEmptyBlocks: boolean
}

export interface FullTonNetworkEnvironmentConfig {
  readonly kind: "fullTonNetwork"
  readonly apiV2Port: number
  readonly apiV3Port: number
  readonly adminPort: number
  readonly validators: number
}

export interface RemoteTonNetworkEnvironmentConfig {
  readonly kind: "remoteTonNetwork"
  readonly network: "testnet" | "mainnet"
}

export type EnvironmentConfig =
  | ActonLocalnetEnvironmentConfig
  | FullTonNetworkEnvironmentConfig
  | RemoteTonNetworkEnvironmentConfig

export interface CreateActonLocalnetEnvironmentConfig {
  readonly kind: "actonLocalnet"
  readonly port?: number
  readonly forkNetwork?: string
  readonly forkBlockNumber?: number
  readonly accounts: readonly string[]
  readonly rateLimit?: number
  readonly responseDelayMs?: number
  readonly blockIntervalMs?: number
  readonly noMining: boolean
  readonly mineEmptyBlocks: boolean
}

export interface CreateFullTonNetworkEnvironmentConfig {
  readonly kind: "fullTonNetwork"
  readonly apiV2Port?: number
  readonly apiV3Port?: number
  readonly adminPort?: number
  readonly validators?: number
}

export type CreateEnvironmentConfig =
  | CreateActonLocalnetEnvironmentConfig
  | CreateFullTonNetworkEnvironmentConfig

export type EnvironmentCapability =
  | "apiV2"
  | "apiV3"
  | "explorer"
  | "integration"
  | "controlApi"
  | "simulator"
  | "wallets"
  | "gramFaucet"
  | "jettonFaucet"
  | "contracts"
  | "apiCalls"
  | "mining"
  | "timeTravel"
  | "snapshots"
  | "checkpoints"

export interface EnvironmentEndpoints {
  readonly apiV2?: string
  readonly apiV3?: string
  readonly control?: string
}

export interface EnvironmentNetwork {
  readonly id: string
  readonly label: string
  readonly chainId: number
  readonly testOnly: boolean
}

export interface StudioEnvironment {
  readonly id: string
  readonly name: string
  readonly status: EnvironmentStatus
  readonly lifecycle: EnvironmentLifecycle
  readonly rpcUrl: string
  readonly config: EnvironmentConfig
  readonly capabilities: readonly EnvironmentCapability[]
  readonly endpoints: EnvironmentEndpoints
  readonly network: EnvironmentNetwork
  readonly error?: string
}

export type StudioWalletVersion = "v4r2" | "v5r1"

export type StudioHex = `0x${string}`

export interface StudioWallet {
  readonly name: string
  readonly address: string
  readonly publicKey: StudioHex
  readonly version: string
  readonly walletId: number
  readonly workchain: number
}

export interface CreateEnvironmentRequest {
  readonly name: string
  readonly config: CreateEnvironmentConfig
}

export interface UpdateEnvironmentRequest {
  readonly name: string
}

export type TestRunSource = "manual" | "studio"
export type TestRunStatus = "queued" | "running" | "passed" | "failed" | "cancelled"

export interface TestRunStats {
  readonly total: number
  readonly passed: number
  readonly failed: number
  readonly skipped: number
  readonly todo: number
  readonly durationMs: number
}

export interface TestRunSummary {
  readonly formatVersion: number
  readonly id: string
  readonly source: TestRunSource
  readonly status: TestRunStatus
  readonly command: readonly string[]
  readonly startedAt: string
  readonly finishedAt?: string
  readonly exitCode?: number
  readonly stats: TestRunStats
  readonly error?: string
}

export interface TestRunRecord extends TestRunSummary {
  readonly projectRoot: string
  readonly reports: readonly TestReport[]
  readonly traceDir?: string
}

export interface StartTestRunRequest {
  readonly paths: readonly string[]
  readonly filter?: string
  readonly include: readonly string[]
  readonly exclude: readonly string[]
  readonly failFast: boolean
  readonly saveTraces: boolean
}

export interface TestRunOutput {
  readonly stdout: string
  readonly stderr: string
}

export type TestRunStreamEvent =
  | {
      readonly type: "runChanged"
      readonly data: {readonly run: TestRunSummary}
    }
  | {
      readonly type: "output"
      readonly data: {
        readonly runId: string
        readonly stream: "stdout" | "stderr"
        readonly chunk: string
      }
    }
  | {
      readonly type: "reporterEvent"
      readonly data: {
        readonly event: {
          readonly runId: string
          readonly sequence: number
          readonly event: {readonly type: string; readonly data: unknown}
        }
      }
    }

export async function fetchStudioInfo(signal: AbortSignal): Promise<StudioInfo> {
  const info = await requestJson<StudioInfo>("/api/v1/info", {
    headers: {accept: "application/json"},
    signal,
  })

  if (info.protocolVersion !== STUDIO_API_VERSION) {
    throw new Error(`Unsupported Studio API version ${info.protocolVersion}`)
  }

  return info
}

export function fetchStudioEnvironments(signal?: AbortSignal): Promise<StudioEnvironment[]> {
  return requestJson<StudioEnvironment[]>("/api/v1/environments", {
    headers: {accept: "application/json"},
    signal,
  })
}

export function createStudioEnvironment(
  request: CreateEnvironmentRequest,
): Promise<StudioEnvironment> {
  return requestJson<StudioEnvironment>("/api/v1/environments", {
    method: "POST",
    headers: {
      accept: "application/json",
      "content-type": "application/json",
    },
    body: JSON.stringify(request),
  })
}

export function stopStudioEnvironment(environmentId: string): Promise<StudioEnvironment> {
  return requestJson<StudioEnvironment>(
    `/api/v1/environments/${encodeURIComponent(environmentId)}/stop`,
    {
      method: "POST",
      headers: {accept: "application/json"},
    },
  )
}

export function restartStudioEnvironment(environmentId: string): Promise<StudioEnvironment> {
  return requestJson<StudioEnvironment>(
    `/api/v1/environments/${encodeURIComponent(environmentId)}/restart`,
    {
      method: "POST",
      headers: {accept: "application/json"},
    },
  )
}

export function updateStudioEnvironment(
  environmentId: string,
  request: UpdateEnvironmentRequest,
): Promise<StudioEnvironment> {
  return requestJson<StudioEnvironment>(
    `/api/v1/environments/${encodeURIComponent(environmentId)}`,
    {
      method: "PATCH",
      headers: {
        accept: "application/json",
        "content-type": "application/json",
      },
      body: JSON.stringify(request),
    },
  )
}

export async function deleteStudioEnvironment(environmentId: string): Promise<void> {
  await request(`/api/v1/environments/${encodeURIComponent(environmentId)}`, {
    method: "DELETE",
    headers: {accept: "application/json"},
  })
}

export function fetchStudioWallets(
  environmentId: string,
  signal?: AbortSignal,
): Promise<StudioWallet[]> {
  return requestJson<StudioWallet[]>(
    `/api/v1/environments/${encodeURIComponent(environmentId)}/wallets`,
    {
      headers: {accept: "application/json"},
      signal,
    },
  )
}

export function signWithStudioWallet(
  environmentId: string,
  walletName: string,
  bytes: StudioHex,
): Promise<{readonly signature: StudioHex}> {
  return requestJson<{readonly signature: StudioHex}>(
    `/api/v1/environments/${encodeURIComponent(environmentId)}/wallets/${encodeURIComponent(walletName)}/sign`,
    {
      method: "POST",
      headers: {
        accept: "application/json",
        "content-type": "application/json",
      },
      body: JSON.stringify({bytes}),
    },
  )
}

export function fetchStudioTestRuns(signal?: AbortSignal): Promise<TestRunSummary[]> {
  return requestJson<TestRunSummary[]>("/api/v1/test-runs", {
    headers: {accept: "application/json"},
    signal,
  })
}

export function fetchStudioTestRun(runId: string, signal?: AbortSignal): Promise<TestRunRecord> {
  return requestJson<TestRunRecord>(`/api/v1/test-runs/${encodeURIComponent(runId)}`, {
    headers: {accept: "application/json"},
    signal,
  })
}

export function startStudioTestRun(request: StartTestRunRequest): Promise<TestRunRecord> {
  return requestJson<TestRunRecord>("/api/v1/test-runs", {
    method: "POST",
    headers: {
      accept: "application/json",
      "content-type": "application/json",
    },
    body: JSON.stringify(request),
  })
}

export function cancelStudioTestRun(runId: string): Promise<TestRunRecord> {
  return requestJson<TestRunRecord>(`/api/v1/test-runs/${encodeURIComponent(runId)}/cancel`, {
    method: "POST",
    headers: {accept: "application/json"},
  })
}

export function fetchStudioTestRunOutput(
  runId: string,
  signal?: AbortSignal,
): Promise<TestRunOutput> {
  return requestJson<TestRunOutput>(`/api/v1/test-runs/${encodeURIComponent(runId)}/output`, {
    headers: {accept: "application/json"},
    signal,
  })
}

export function studioTestRunArtifactsUrl(runId: string) {
  return `/api/v1/test-runs/${encodeURIComponent(runId)}/artifacts`
}

export function subscribeToStudioTestRuns(
  onEvent: (event: TestRunStreamEvent) => void,
  onError?: () => void,
  onOpen?: () => void,
) {
  const source = new EventSource("/api/v1/test-runs/events")
  source.addEventListener("test-run", event => {
    try {
      onEvent(JSON.parse(event.data) as TestRunStreamEvent)
    } catch {
      // A malformed live event must not break history polling.
    }
  })
  if (onError) source.addEventListener("error", onError)
  if (onOpen) source.addEventListener("open", onOpen)
  return () => source.close()
}

export async function requestJson<T>(input: string, init?: RequestInit): Promise<T> {
  const response = await request(input, init)
  return (await response.json()) as T
}

async function request(input: string, init?: RequestInit): Promise<Response> {
  const response = await fetch(input, init)
  if (response.ok) return response

  const fallbackMessage = `Studio server returned ${response.status}`
  let body: {readonly error?: {readonly message?: string}}
  try {
    body = (await response.json()) as typeof body
  } catch (error) {
    throw new Error(fallbackMessage, {cause: error})
  }
  throw new Error(body.error?.message || fallbackMessage)
}
