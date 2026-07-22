import {TxTraceError} from "./tx-trace-error"

export class TooManyRequests extends TxTraceError {
  constructor(message = "Too many requests, please try again a bit later", cause?: unknown) {
    super(message, cause)
    this.name = "TooManyRequests"
  }
}
