export class TxTraceError extends Error {
  constructor(
    message: string,
    public readonly cause?: unknown,
  ) {
    super(message)
    this.name = "TxTraceError"
  }
}

export class TxNotFoundError extends TxTraceError {
  constructor(message = "Transaction not found", cause?: unknown) {
    super(message, cause)
    this.name = "TxNotFoundError"
  }
}

export class TxHashInvalidError extends TxTraceError {
  constructor(message = "Transaction hash is not valid", cause?: unknown) {
    super(message, cause)
    this.name = "TxHashInvalidError"
  }
}

export class NetworkError extends TxTraceError {
  constructor(message = "Network error", cause?: unknown) {
    super(message, cause)
    this.name = "NetworkError"
  }
}

export class TooManyRequests extends TxTraceError {
  constructor(message = "Too many requests, please try again a bit later", cause?: unknown) {
    super(message, cause)
    this.name = "TooManyRequests"
  }
}
