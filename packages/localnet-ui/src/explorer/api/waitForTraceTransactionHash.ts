import type {TonClient} from "./client"

export async function waitForTraceTransactionHash(
  client: TonClient,
  msgHash: string,
): Promise<string | undefined> {
  for (let attempt = 0; attempt < 8; attempt += 1) {
    if (attempt > 0) {
      await delay(500)
    }

    try {
      const response = await client.getTracesByMessageHash(msgHash)
      const txHash = response.traces[0]?.trace.tx_hash ?? response.traces[0]?.transactions_order[0]
      if (txHash) {
        return txHash
      }
    } catch {
      // A message can be accepted before the next scheduled block indexes its trace.
    }
  }

  return undefined
}

function delay(durationMs: number): Promise<void> {
  return new Promise(resolve => {
    globalThis.setTimeout(resolve, durationMs)
  })
}
