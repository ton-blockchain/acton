import initFaucetPow, {find_nonce} from "./wasm/acton_faucet_pow_wasm"
import {
  solveFaucetChallenge,
  type FaucetPowRequest,
  type FaucetPowWorkerResponse,
} from "./faucetPow"

interface FaucetPowWorkerScope {
  onmessage: ((event: MessageEvent<FaucetPowRequest>) => void) | null
  postMessage(message: FaucetPowWorkerResponse): void
}

const workerScope = globalThis as unknown as FaucetPowWorkerScope
let wasmReady: Promise<unknown> | undefined

workerScope.onmessage = event => {
  void solve(event.data)
}

async function solve(request: FaucetPowRequest): Promise<void> {
  try {
    wasmReady ??= initFaucetPow()
    await wasmReady
    const solution = solveFaucetChallenge(
      request,
      (challenge, difficulty, startNonce, maxAttempts) =>
        find_nonce(challenge, difficulty, startNonce, maxAttempts),
      progress => {
        workerScope.postMessage({type: "progress", progress})
      },
    )
    workerScope.postMessage({type: "solved", solution})
  } catch (error) {
    workerScope.postMessage({
      type: "error",
      message: error instanceof Error ? error.message : "Failed to solve PoW challenge",
    })
  }
}
