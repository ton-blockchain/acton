import {describe, expect, test} from "bun:test"

import {solveFaucetChallenge} from "../src/faucet/faucetPow"

describe("faucet proof of work", () => {
  test("returns a nonce found by the WASM chunk scanner", () => {
    const solution = solveFaucetChallenge(
      {
        challenge: "actonscan-test-vector",
        difficulty: 12,
        maxSolveTtlSeconds: 10,
        maxNonceAttempts: 100_000,
      },
      (challenge, difficulty, startNonce, maxAttempts) => {
        expect(challenge).toBe("actonscan-test-vector")
        expect(difficulty).toBe(12)
        expect(startNonce).toBe(0)
        expect(maxAttempts).toBe(100_000)
        return 3869
      },
    )

    expect(solution.nonce).toBe(3869)
    expect(solution.attempts).toBe(3870)
  })

  test("scans consecutive chunks until the WASM scanner finds a nonce", () => {
    const starts: number[] = []
    const solution = solveFaucetChallenge(
      {
        challenge: "challenge",
        difficulty: 20,
        maxSolveTtlSeconds: 10,
        maxNonceAttempts: 3_000_000,
      },
      (_challenge, _difficulty, startNonce) => {
        starts.push(startNonce)
        return startNonce === 2_097_152 ? 2_500_000 : -1
      },
    )

    expect(starts).toEqual([0, 1_048_576, 2_097_152])
    expect(solution.nonce).toBe(2_500_000)
    expect(solution.attempts).toBe(2_500_001)
  })

  test("rejects unsafe nonce limits before solving", () => {
    expect(() =>
      solveFaucetChallenge(
        {
          challenge: "challenge",
          difficulty: 1,
          maxSolveTtlSeconds: 10,
          maxNonceAttempts: Number.MAX_SAFE_INTEGER + 1,
        },
        () => -1,
      ),
    ).toThrow("positive safe integer")
  })
})
