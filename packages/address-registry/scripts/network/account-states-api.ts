import type {AccountStatusEntry} from "./account-statuses.ts"

const MAINNET_ACCOUNT_STATES_URL = "https://toncenter.com/api/v3/accountStates"
const TESTNET_ACCOUNT_STATES_URL = "https://testnet.toncenter.com/api/v3/accountStates"
const BATCH_SIZE = 100
const UNAUTHENTICATED_REQUEST_DELAY_MS = 1100
const ACCOUNT_STATES_RETRY_COUNT = 3
const ACCOUNT_STATES_RETRY_DELAY_MS = 2000

interface AccountStatesResponse {
  readonly accounts: readonly AccountStatusEntry[]
}

export interface NetworkAccountStates {
  readonly mainnet: readonly AccountStatusEntry[]
  readonly testnet: readonly AccountStatusEntry[]
}

const wait = (milliseconds: number): Promise<void> =>
  new Promise(resolve => setTimeout(resolve, milliseconds))

const readAccountStates = async (
  endpoint: string,
  addresses: readonly string[],
  apiKey: string | undefined,
  retry = 0,
): Promise<readonly AccountStatusEntry[]> => {
  const parameters = new URLSearchParams()
  parameters.set("include_boc", "false")
  for (const address of addresses) {
    parameters.append("address", address)
  }

  const response = await fetch(`${endpoint}?${parameters}`, {
    headers: apiKey ? {"X-API-Key": apiKey} : undefined,
  })
  if (response.status === 429 && retry < ACCOUNT_STATES_RETRY_COUNT) {
    const retryAfterHeader = response.headers.get("Retry-After")
    const retryAfterSeconds = retryAfterHeader === null ? Number.NaN : Number(retryAfterHeader)
    const retryDelay =
      Number.isFinite(retryAfterSeconds) && retryAfterSeconds >= 0
        ? retryAfterSeconds * 1000
        : ACCOUNT_STATES_RETRY_DELAY_MS * 2 ** retry
    await wait(retryDelay)
    return readAccountStates(endpoint, addresses, apiKey, retry + 1)
  }
  if (!response.ok) {
    throw new Error(`Failed to read account states from ${endpoint}: HTTP ${response.status}`)
  }

  const result = (await response.json()) as AccountStatesResponse
  return result.accounts
}

export const readNetworkAccountStates = async (
  addresses: readonly string[],
  apiKey: string | undefined,
): Promise<NetworkAccountStates> => {
  const mainnet: AccountStatusEntry[] = []
  const testnet: AccountStatusEntry[] = []

  for (let offset = 0; offset < addresses.length; offset += BATCH_SIZE) {
    const batch = addresses.slice(offset, offset + BATCH_SIZE)
    // Network requests within a batch run in parallel; batches remain sequential for rate limiting.
    // biome-ignore lint/performance/noAwaitInLoops: the endpoints are intentionally rate-limited
    const [mainnetBatch, testnetBatch] = await Promise.all([
      readAccountStates(MAINNET_ACCOUNT_STATES_URL, batch, apiKey),
      readAccountStates(TESTNET_ACCOUNT_STATES_URL, batch, apiKey),
    ])
    mainnet.push(...mainnetBatch)
    testnet.push(...testnetBatch)

    if (!apiKey && offset + BATCH_SIZE < addresses.length) {
      await wait(UNAUTHENTICATED_REQUEST_DELAY_MS)
    }
  }

  return {mainnet, testnet}
}
