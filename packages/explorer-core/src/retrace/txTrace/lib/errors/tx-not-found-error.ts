import {TxTraceError} from "./tx-trace-error"

export class TxNotFoundError extends TxTraceError {
  constructor(message = "Transaction not found", cause?: unknown) {
    super(message, cause)
    this.name = "TxNotFoundError"
  }
}
