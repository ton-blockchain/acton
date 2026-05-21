export class ActonError extends Error {
  constructor(message: string) {
    super(message)
    this.name = "ActonError"
  }
}

export class LocalnetApiError extends ActonError {
  readonly status?: number
  readonly code?: number

  constructor(message: string, options: {readonly status?: number; readonly code?: number} = {}) {
    super(message)
    this.name = "LocalnetApiError"
    this.status = options.status
    this.code = options.code
  }
}

export function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error)
}
