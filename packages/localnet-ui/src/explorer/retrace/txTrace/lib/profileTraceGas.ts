import {
  buildGasProfile,
  type GasProfileExecutionRequest,
  type GasProfileResponse,
  type TraceReplayResult,
  type TraceResult,
} from "@ton/retracer-core"
import type {GasProfileContract, GasProfileData} from "@acton/transaction-ui"

import type {ExplorerMetadataRegistry} from "../../../metadata/types"
import {loadVerifiedTolkSource, verifiedSourceTraceOptions} from "./traceTx"

interface GasProfileGroup {
  readonly codeHash: string
  readonly contractName: string
  readonly compiled: NonNullable<
    ReturnType<typeof verifiedSourceTraceOptions>
  >["sourceMap"]
  readonly executions: readonly GasProfileExecutionRequest[]
}

export interface TraceGasProfile {
  readonly profile: GasProfileData
  readonly profiledTransactions: number
  readonly totalTransactions: number
}

export async function profileTraceGas(
  trace: TraceReplayResult,
  metadataRegistry: ExplorerMetadataRegistry,
): Promise<TraceGasProfile> {
  const transactionsByCodeHash = groupTransactionsByCodeHash(trace.transactions)
  const groups = (
    await Promise.all(
      [...transactionsByCodeHash.entries()].map(([codeHash, transactions]) =>
        loadGasProfileGroup(metadataRegistry, codeHash, transactions),
      ),
    )
  ).filter(group => group !== undefined)

  const responses = await Promise.all(
    groups.map(async group => ({
      group,
      response: await buildGasProfile({
        codeHash: group.codeHash,
        compiled: group.compiled,
        executions: group.executions,
      }),
    })),
  )
  const contracts = responses.map(({group, response}) =>
    gasProfileContract(group.contractName, response),
  )

  return {
    profile: {
      total_gas: contracts.reduce((total, contract) => total + contract.total_gas, 0),
      contracts,
    },
    profiledTransactions: groups.reduce((total, group) => total + group.executions.length, 0),
    totalTransactions: Object.keys(trace.transactions).length,
  }
}

function groupTransactionsByCodeHash(
  transactions: TraceReplayResult["transactions"],
): Map<string, [string, TraceResult][]> {
  const groups = new Map<string, [string, TraceResult][]>()

  for (const [transactionHash, transaction] of Object.entries(transactions)) {
    const codeHash = transaction.codeCell?.hash().toString("hex")
    if (!codeHash || !transaction.emulatedTx.vmLogs) continue

    const group = groups.get(codeHash) ?? []
    group.push([transactionHash, transaction])
    groups.set(codeHash, group)
  }

  return groups
}

async function loadGasProfileGroup(
  metadataRegistry: ExplorerMetadataRegistry,
  codeHash: string,
  transactions: readonly [string, TraceResult][],
): Promise<GasProfileGroup | undefined> {
  const verifiedSource = await loadVerifiedTolkSource(metadataRegistry, codeHash)
  const sourceTraceOptions = verifiedSourceTraceOptions(verifiedSource)
  if (!sourceTraceOptions || !verifiedSource?.bundle) return

  const contractName =
    verifiedSource.bundle.entrypoint.split("/").pop()?.replace(/\.tolk$/i, "") ||
    `Contract ${codeHash.slice(0, 8)}`

  return {
    codeHash,
    contractName,
    compiled: sourceTraceOptions.sourceMap,
    executions: transactions.map(([transactionHash, transaction]) => ({
      id: transactionHash,
      vmLogs: transaction.emulatedTx.vmLogs,
      initialGas: transaction.emulatedTx.initialGas,
      contractName,
    })),
  }
}

function gasProfileContract(
  contractName: string,
  response: GasProfileResponse,
): GasProfileContract {
  return {
    name: contractName,
    total_gas: response.executions.reduce(
      (total, execution) => total + execution.total_gas,
      0,
    ),
    sample_count: response.executions.reduce(
      (total, execution) => total + execution.sample_count,
      0,
    ),
    samples: response.executions.flatMap(execution => execution.samples),
  }
}
