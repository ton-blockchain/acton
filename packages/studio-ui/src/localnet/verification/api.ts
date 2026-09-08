import {requestJson} from "../../studioApi"

/** Public verification is independent of matching a local source artifact */
export interface VerificationStatus {
  readonly codeHash: string
  readonly verified: boolean
  readonly verifierUrl: string
}

export interface VerificationCandidate {
  readonly contractId: string
  readonly sourcePath: string
  readonly codeHash: string | null
  readonly matches: boolean
  readonly error: string | null
  readonly files: readonly {readonly path: string; readonly sizeBytes: number}[]
}

export interface VerificationPreview {
  readonly id: string
  readonly status: VerificationStatus
  readonly compilerVersion: string
  readonly candidates: readonly VerificationCandidate[]
  readonly payment: {
    readonly network: "testnet"
    readonly address: string
    readonly amount: string
    readonly comment: string
  } | null
}

export interface VerificationOperation {
  readonly id: string
  readonly phase: "uploadingSources" | "confirmingPayment" | "ready" | "verified" | "failed"
  readonly message: {
    readonly address: string
    readonly amount: string
    readonly payload: string
  } | null
  readonly error: string | null
}

/** All calls stay scoped to the chosen Studio environment, including operation polling */
export function verificationApi(environmentId: string) {
  const base = `/api/v1/environments/${encodeURIComponent(environmentId)}/verification`

  return {
    status: (codeHash: string, signal?: AbortSignal) =>
      requestJson<VerificationStatus>(`${base}/status?${new URLSearchParams({codeHash})}`, {
        signal,
      }),

    preview: (codeHash: string) =>
      requestJson<VerificationPreview>(`${base}/preview`, {
        method: "POST",
        headers: {"Content-Type": "application/json"},
        body: JSON.stringify({codeHash}),
      }),

    start: (previewId: string, contractId: string, senderAddress: string) =>
      requestJson<VerificationOperation>(`${base}/operations`, {
        method: "POST",
        headers: {"Content-Type": "application/json"},
        body: JSON.stringify({previewId, contractId, senderAddress}),
      }),

    operation: (id: string) =>
      requestJson<VerificationOperation>(`${base}/operations/${encodeURIComponent(id)}`),

    completePayment: (id: string, messageHash: string) =>
      requestJson<VerificationOperation>(`${base}/operations/${encodeURIComponent(id)}/payment`, {
        method: "POST",
        headers: {"Content-Type": "application/json"},
        body: JSON.stringify({messageHash}),
      }),
  }
}
