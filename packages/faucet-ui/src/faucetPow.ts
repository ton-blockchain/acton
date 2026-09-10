const POW_CHUNK_SIZE = 1_048_576

export interface FaucetPowRequest {
  readonly challenge: string
  readonly difficulty: number
  readonly maxSolveTtlSeconds: number
  readonly maxNonceAttempts: number
}

export interface FaucetPowProgress {
  readonly attempts: number
  readonly elapsedMs: number
}

export interface FaucetPowSolution extends FaucetPowProgress {
  readonly nonce: number
}

export type FaucetPowWorkerResponse =
  | {readonly type: "progress"; readonly progress: FaucetPowProgress}
  | {readonly type: "solved"; readonly solution: FaucetPowSolution}
  | {readonly type: "error"; readonly message: string}

export type FindNonceInChunk = (
  challenge: string,
  difficulty: number,
  startNonce: number,
  maxAttempts: number,
) => number

export function solveFaucetChallenge(
  request: FaucetPowRequest,
  findNonceInChunk: FindNonceInChunk,
  onProgress?: (progress: FaucetPowProgress) => void,
): FaucetPowSolution {
  validatePowRequest(request)

  const startedAt = performance.now()
  let attempts = 0

  while (attempts < request.maxNonceAttempts) {
    const elapsedMs = performance.now() - startedAt
    if (elapsedMs >= request.maxSolveTtlSeconds * 1000) {
      throw new Error(`PoW solve exceeded time limit of ${request.maxSolveTtlSeconds}s`)
    }
    onProgress?.({attempts, elapsedMs})

    const chunkSize = Math.min(POW_CHUNK_SIZE, request.maxNonceAttempts - attempts)
    const nonce = findNonceInChunk(request.challenge, request.difficulty, attempts, chunkSize)
    if (nonce >= 0) {
      if (!Number.isSafeInteger(nonce) || nonce < attempts || nonce >= attempts + chunkSize) {
        throw new Error("PoW solver returned an invalid nonce")
      }
      return {
        nonce,
        attempts: nonce + 1,
        elapsedMs: performance.now() - startedAt,
      }
    }

    attempts += chunkSize
  }

  throw new Error(`PoW solve exceeded nonce limit of ${request.maxNonceAttempts}`)
}

export function validatePowRequest(request: FaucetPowRequest): void {
  if (!Number.isInteger(request.difficulty) || request.difficulty < 0 || request.difficulty > 256) {
    throw new Error("PoW difficulty must be between 0 and 256 bits")
  }
  if (!Number.isSafeInteger(request.maxNonceAttempts) || request.maxNonceAttempts <= 0) {
    throw new Error("PoW nonce limit must be a positive safe integer")
  }
  if (!Number.isFinite(request.maxSolveTtlSeconds) || request.maxSolveTtlSeconds <= 0) {
    throw new Error("PoW solve time limit must be positive")
  }
}
