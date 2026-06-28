import type {TraceInfo} from "@ton/tasm/dist/trace"
import type {TraceResult} from "@ton/retracer-core"

import type {InstructionInfo} from "ton-source-map"

import type {SourceTraceResponse, VerificationSourceResponse} from "../../../api/types"
import type {ExplorerNetworkInfo} from "../../../hooks/useNetworkInfo"

export type ExitCode = {
  readonly num: number
  readonly description: string
  readonly info: InstructionInfo | undefined
  readonly vmPosition?: {
    readonly cellHash: string
    readonly offset: number
  }
}

export interface RetraceResultAndCode {
  readonly result: TraceResult
  readonly code: string
  readonly trace: TraceInfo
  readonly exitCode: ExitCode | undefined
  readonly network: ExplorerNetworkInfo
  readonly verifiedSource?: VerificationSourceResponse
  readonly sourceTrace?: SourceTraceResponse
  readonly sourceTraceBundleHash?: string
}

export interface InstructionDetail {
  readonly name: string
  readonly gasCost: number
  readonly instructionText?: string
}
