import {TxTraceError} from "./tx-trace-error"

export class NetworkError extends TxTraceError {
  constructor(message = "Network error", cause?: unknown) {
    super(message, cause)
    this.name = "NetworkError"
  }
}
