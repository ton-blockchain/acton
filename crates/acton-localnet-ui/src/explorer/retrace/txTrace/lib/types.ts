import type {TraceInfo} from "ton-assembly/dist/trace"
import type {TraceResult} from "@ton/retracer-core"

import type {InstructionInfo} from "ton-source-map"

import type {ExplorerNetworkInfo} from "../../../hooks/useNetworkInfo"

export type ExitCode = {
  readonly num: number
  readonly description: string
  readonly info: InstructionInfo | undefined
}

export interface RetraceResultAndCode {
  readonly result: TraceResult
  readonly code: string
  readonly trace: TraceInfo
  readonly exitCode: ExitCode | undefined
  readonly network: ExplorerNetworkInfo
}

export interface InstructionDetail {
  readonly name: string
  readonly gasCost: number
  readonly instructionText?: string
}
