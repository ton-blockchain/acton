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

export interface ActonSimulatedLocalnetEnvironmentConfig {
  readonly kind: "actonSimulatedLocalnet"
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
  readonly configPort: number
  readonly observabilityPort: number
  readonly blockTimeMs?: number
  readonly electionTimeSeconds?: number
  readonly importedAccounts: readonly FullTonAccountImport[]
  readonly nodes: readonly FullTonNode[]
}

export interface FullTonNode {
  readonly id: string
  readonly name: string
  readonly validator: boolean
  readonly portBase: number
}

export interface FullTonAccountImport {
  readonly sourceEnvironmentId: string
  readonly address: string
  readonly name?: string
}

export interface RemoteTonNetworkEnvironmentConfig {
  readonly kind: "remoteTonNetwork"
  readonly network: "testnet" | "mainnet"
}

export type EnvironmentConfig =
  | ActonSimulatedLocalnetEnvironmentConfig
  | FullTonNetworkEnvironmentConfig
  | RemoteTonNetworkEnvironmentConfig

export interface CreateActonSimulatedLocalnetEnvironmentConfig {
  readonly kind: "actonSimulatedLocalnet"
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
  readonly configPort?: number
  readonly observabilityPort?: number
  readonly blockTimeMs?: number
  readonly electionTimeSeconds?: number
  readonly importedAccounts: readonly FullTonAccountImport[]
}

export type CreateEnvironmentConfig =
  | CreateActonSimulatedLocalnetEnvironmentConfig
  | CreateFullTonNetworkEnvironmentConfig

export type EnvironmentCapability =
  | "apiV2"
  | "apiV3"
  | "configApi"
  | "explorer"
  | "integration"
  | "testnetFaucet"
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
  | "observability"
  | "health"

export type NetworkHealthStatus = "healthy" | "syncing" | "degraded" | "stopped"
export type ApiHealthStatus = "ready" | "syncing" | "unavailable" | "stopped"
export type ServiceHealthStatus =
  | "ready"
  | "starting"
  | "completed"
  | "stopped"
  | "failed"
  | "unknown"

export interface ApiHealth {
  readonly status: ApiHealthStatus
  readonly endpoint: string
  readonly latencyMs: number | null
  readonly masterchainSeqno: number | null
  readonly blockTimeUnix: number | null
  readonly blockAgeMs: number | null
  readonly error: string | null
}

export interface ServiceHealth {
  readonly name: string
  readonly status: ServiceHealthStatus
  readonly state: string | null
  readonly health: string | null
  readonly exitCode: number | null
}

export interface NetworkHealthSample {
  readonly observedAtMs: number
  readonly apiV2LatencyMs: number | null
  readonly apiV3LatencyMs: number | null
  readonly apiV2Seqno: number | null
  readonly apiV3Seqno: number | null
  readonly indexerLagBlocks: number | null
  readonly blockAgeMs: number | null
}

export interface NetworkHealth {
  readonly observedAtMs: number
  readonly status: NetworkHealthStatus
  readonly apiV2: ApiHealth
  readonly apiV3: ApiHealth
  readonly indexerLagBlocks: number | null
  readonly estimatedIndexerLagMs: number | null
  readonly services: readonly ServiceHealth[]
  readonly history: readonly NetworkHealthSample[]
  readonly infrastructureError: string | null
}

export interface EnvironmentEndpoints {
  readonly apiV2?: string
  readonly apiV3?: string
  readonly config?: string
  readonly control?: string
  readonly observability?: string
}

export interface EnvironmentNetwork {
  readonly id: string
  readonly label: string
  readonly chainId: number
  readonly testOnly: boolean
  readonly supportsActions: boolean
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
  readonly startupTimings?: EnvironmentStartupTimings
}

export type ApiCallStatus = "success" | "failed"
export type ApiCallType = "read" | "write"
export type ApiCallSource = "external" | "studio_ui"
export type ApiCallFamily = "control" | "emulate" | "json_rpc" | "streaming" | "v2" | "v3"

export interface ApiCallRecord {
  readonly sequence: number
  readonly status: ApiCallStatus
  readonly status_code: number
  readonly source: ApiCallSource
  readonly call_type: ApiCallType
  readonly api_family: ApiCallFamily
  readonly http_method: string
  readonly path: string
  readonly method: string
  readonly request_id: unknown
  readonly query_params: unknown | null
  readonly request_body: unknown | null
  readonly request_body_truncated: boolean
  readonly response_body: unknown | null
  readonly response_body_truncated: boolean
  readonly timestamp_ms: number
  readonly duration_ns: number
}

export interface ApiCallLogResponse {
  readonly calls: readonly ApiCallRecord[]
  readonly total_retained: number
  readonly max_retained: number
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

export interface CreateFullTonNodeRequest {
  readonly name: string
  readonly validator: boolean
}

export interface EnvironmentSnapshot {
  readonly formatVersion: number
  readonly id: string
  readonly name?: string
  readonly createdAt: number
  readonly archiveSizeBytes: number
  readonly stateSizeBytes: number
  readonly stateSchemaVersion: number
  readonly tonRelease: string
  readonly masterchainSeqno?: number
}

export type EnvironmentSnapshotOperationKind = "create" | "restore"
export type EnvironmentSnapshotOperationPhase =
  | "preparing"
  | "stopping"
  | "creatingArchive"
  | "restoringState"
  | "resettingIndexer"
  | "starting"
  | "completed"
  | "failed"

export interface EnvironmentStartupTimings {
  readonly composeMs?: number
  readonly tonReadyMs?: number
  readonly indexerReadyMs?: number
  readonly apiReadyMs?: number
}

export interface EnvironmentSnapshotOperation {
  readonly kind: EnvironmentSnapshotOperationKind
  readonly phase: EnvironmentSnapshotOperationPhase
  readonly startedAt: string
  readonly finishedAt?: string
  readonly snapshotId?: string
  readonly snapshotName?: string
  readonly startupTimings?: EnvironmentStartupTimings
  readonly error?: string
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

/** Checks whether the Studio owner process still serves its control API. */
export async function checkStudioConnection(signal: AbortSignal): Promise<void> {
  // The health endpoint intentionally returns 204, so parsing it as JSON would
  // turn every successful heartbeat into a false connection failure.
  await request("/api/v1/health", {
    cache: "no-store",
    signal,
  })
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

export function fetchStudioEnvironmentHealth(
  environmentId: string,
  signal?: AbortSignal,
): Promise<NetworkHealth> {
  return requestJson<NetworkHealth>(
    `/api/v1/environments/${encodeURIComponent(environmentId)}/health`,
    {headers: {accept: "application/json"}, signal},
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

export function addStudioFullTonNode(
  environmentId: string,
  request: CreateFullTonNodeRequest,
): Promise<StudioEnvironment> {
  return requestJson<StudioEnvironment>(
    `/api/v1/environments/${encodeURIComponent(environmentId)}/nodes`,
    {
      method: "POST",
      headers: {
        accept: "application/json",
        "content-type": "application/json",
      },
      body: JSON.stringify(request),
    },
  )
}

export function removeStudioFullTonNode(
  environmentId: string,
  nodeId: string,
): Promise<StudioEnvironment> {
  return requestJson<StudioEnvironment>(
    `/api/v1/environments/${encodeURIComponent(environmentId)}/nodes/${encodeURIComponent(nodeId)}`,
    {
      method: "DELETE",
      headers: {accept: "application/json"},
    },
  )
}

export function leaveStudioFullTonValidation(
  environmentId: string,
  nodeId: string,
): Promise<StudioEnvironment> {
  return requestJson<StudioEnvironment>(
    `/api/v1/environments/${encodeURIComponent(environmentId)}/nodes/${encodeURIComponent(nodeId)}/leave-validation`,
    {
      method: "POST",
      headers: {accept: "application/json"},
    },
  )
}

export function enterStudioFullTonValidation(
  environmentId: string,
  nodeId: string,
): Promise<StudioEnvironment> {
  return requestJson<StudioEnvironment>(
    `/api/v1/environments/${encodeURIComponent(environmentId)}/nodes/${encodeURIComponent(nodeId)}/enter-validation`,
    {
      method: "POST",
      headers: {accept: "application/json"},
    },
  )
}

export async function deleteStudioEnvironment(environmentId: string): Promise<void> {
  await request(`/api/v1/environments/${encodeURIComponent(environmentId)}`, {
    method: "DELETE",
    headers: {accept: "application/json"},
  })
}

export function fetchStudioEnvironmentSnapshots(
  environmentId: string,
  signal?: AbortSignal,
): Promise<EnvironmentSnapshot[]> {
  return requestJson<EnvironmentSnapshot[]>(
    `/api/v1/environments/${encodeURIComponent(environmentId)}/snapshots`,
    {headers: {accept: "application/json"}, signal},
  )
}

export function createStudioEnvironmentSnapshot(
  environmentId: string,
  name?: string,
): Promise<EnvironmentSnapshotOperation> {
  return requestJson<EnvironmentSnapshotOperation>(
    `/api/v1/environments/${encodeURIComponent(environmentId)}/snapshots`,
    {
      method: "POST",
      headers: {accept: "application/json", "content-type": "application/json"},
      body: JSON.stringify({name}),
    },
  )
}

export function restoreStudioEnvironmentSnapshot(
  environmentId: string,
  snapshotId: string,
): Promise<EnvironmentSnapshotOperation> {
  return requestJson<EnvironmentSnapshotOperation>(
    `/api/v1/environments/${encodeURIComponent(environmentId)}/snapshots/${encodeURIComponent(snapshotId)}/restore`,
    {method: "POST", headers: {accept: "application/json"}},
  )
}

export async function deleteStudioEnvironmentSnapshot(
  environmentId: string,
  snapshotId: string,
): Promise<void> {
  await request(
    `/api/v1/environments/${encodeURIComponent(environmentId)}/snapshots/${encodeURIComponent(snapshotId)}`,
    {method: "DELETE", headers: {accept: "application/json"}},
  )
}

export function fetchStudioEnvironmentSnapshotOperation(
  environmentId: string,
  signal?: AbortSignal,
): Promise<EnvironmentSnapshotOperation | null> {
  return requestJson<EnvironmentSnapshotOperation | null>(
    `/api/v1/environments/${encodeURIComponent(environmentId)}/snapshot-operation`,
    {headers: {accept: "application/json"}, signal},
  )
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

export function fetchStudioApiCalls(
  environmentId: string,
  limit = 1200,
  signal?: AbortSignal,
): Promise<ApiCallLogResponse> {
  const query = new URLSearchParams({limit: String(limit)})
  return requestJson<ApiCallLogResponse>(
    `/api/v1/environments/${encodeURIComponent(environmentId)}/api-calls?${query}`,
    {
      headers: {accept: "application/json"},
      signal,
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

export class StudioRequestError extends Error {
  constructor(
    message: string,
    readonly status: number,
  ) {
    super(message)
  }
}

async function request(input: string, init?: RequestInit): Promise<Response> {
  const response = await fetch(input, init)
  if (response.ok) return response

  const fallbackMessage = `Studio server returned ${response.status}`
  let body: {readonly error?: {readonly message?: string}}
  try {
    body = (await response.json()) as typeof body
  } catch {
    throw new StudioRequestError(fallbackMessage, response.status)
  }
  throw new StudioRequestError(body.error?.message || fallbackMessage, response.status)
}

export type AdminAccountChange =
  | {readonly type: "balance"; readonly balance: string}
  | {readonly type: "code" | "data" | "replace"; readonly boc: string}
  | {readonly type: "freeze" | "delete"}
  | {readonly type: "uninit"; readonly balance?: string}

export type AdminRequest =
  | {
      readonly kind: "accounts"
      readonly id: string
      readonly edits: readonly ({readonly address: string} & AdminAccountChange)[]
    }
  | {readonly kind: "config"; readonly id: string; readonly index: number; readonly boc: string}

export interface AdminOperation {
  readonly id: string
  readonly phase: string
  readonly startedAt: string
  readonly finishedAt: string | null
  readonly error: string | null
  readonly blockSeqno: number | null
}

export function fetchStudioAdminOperation(environmentId: string, signal?: AbortSignal) {
  return requestJson<AdminOperation | null>(
    `/api/v1/environments/${encodeURIComponent(environmentId)}/admin`,
    {signal},
  )
}

export function startStudioAdminOperation(environmentId: string, request: AdminRequest) {
  return requestJson<AdminOperation>(
    `/api/v1/environments/${encodeURIComponent(environmentId)}/admin`,
    {
      method: "POST",
      headers: {"Content-Type": "application/json"},
      body: JSON.stringify(request),
    },
  )
}
