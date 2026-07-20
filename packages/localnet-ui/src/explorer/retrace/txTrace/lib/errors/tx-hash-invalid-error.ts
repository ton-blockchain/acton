import {TxTraceError} from "./tx-trace-error"

export class TxHashInvalidError extends TxTraceError {
  constructor(message = "Transaction hash is not valid", cause?: unknown) {
    super(message, cause)
    this.name = "TxHashInvalidError"
  }
}
