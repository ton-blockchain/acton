import {
  buildValueFlowItems,
  decodeStorageDataCell,
  type ContractData,
  type ContractVerifiedSource,
  type TransactionInfo,
  type ValueFlowItem,
} from "@acton/shared-ui"
import {Address} from "@ton/core"

import type {TonClient} from "../api/client"
import {addressKey} from "../api/compilerAbi"
import {resolveCompilerAbis} from "../api/compilerAbiResolver"
import type {V3Transaction} from "../api/types"
import {
  formatAddress as formatDisplayAddress,
  normalizeAddress,
} from "../components/utils"
import type {ExplorerMetadataRegistry} from "../metadata/types"

type AddressFormat = Parameters<typeof normalizeAddress>[1]

export interface TraceTransactionEnrichmentOptions {
  readonly client: TonClient
  readonly metadataRegistry: ExplorerMetadataRegistry
  readonly transactions: readonly TransactionInfo[]
  readonly transactionsMap: Record<string, V3Transaction>
  readonly fetchName: (address: string) => Promise<string | undefined>
  readonly addressFormat: AddressFormat
  readonly shouldContinue?: () => boolean
}

export interface TraceTransactionEnrichmentResult {
  readonly transactions: TransactionInfo[]
  readonly contracts: Map<string, ContractData>
  readonly compilerAbisByCodeHash: Map<string, ContractData["abi"]>
  readonly verifiedSourcesByCodeHash: Map<string, ContractVerifiedSource>
  readonly valueFlow: ValueFlowItem[]
}

export async function enrichTraceTransactions({
  client,
  metadataRegistry,
  transactions,
  transactionsMap,
  fetchName,
  addressFormat,
  shouldContinue = () => true,
}: TraceTransactionEnrichmentOptions): Promise<TraceTransactionEnrichmentResult | undefined> {
  const processed = [...transactions]
  const transactionsByLt = new Map(
    Object.values(transactionsMap).map(tx => [tx.lt, tx] as const),
  )
  const traceAddressOrder = collectTraceAddressOrder(processed)
  const requestedAddresses = [...traceAddressOrder].sort()
  const additionalCodeHashes = new Set<string>()

  for (const tx of Object.values(transactionsMap)) {
    if (tx.account_state_before?.code_hash) {
      additionalCodeHashes.add(tx.account_state_before.code_hash)
    }
    if (tx.account_state_after?.code_hash) {
      additionalCodeHashes.add(tx.account_state_after.code_hash)
    }
  }

  const stateInitCodeHashes = new Set<string>()
  for (const tx of processed) {
    const stateInitCodeHash = tx.transaction.inMessage?.init?.code?.hash().toString("hex")
    if (stateInitCodeHash) {
      additionalCodeHashes.add(stateInitCodeHash)
      stateInitCodeHashes.add(stateInitCodeHash)
    }
  }

  const resolvedAbis = await resolveCompilerAbis({
    client,
    metadataRegistry,
    addresses: requestedAddresses,
    additionalCodeHashes: [...additionalCodeHashes],
    shouldContinue,
  })
  if (!resolvedAbis) {
    return undefined
  }

  const {addressToCodeHash, abiByCodeHash} = resolvedAbis
  const verifiedSourcesByCodeHash = await loadVerifiedSourcesByCodeHash({
    metadataRegistry,
    codeHashes: [...stateInitCodeHashes],
    shouldContinue,
  })
  if (!verifiedSourcesByCodeHash) {
    return undefined
  }

  for (const tx of processed) {
    const sourceTx = transactionsByLt.get(tx.lt)
    const fallbackCodeHash = tx.address
      ? addressToCodeHash.get(addressKey(tx.address.toString()))
      : undefined
    const beforeCodeHash = sourceTx?.account_state_before?.code_hash ?? fallbackCodeHash
    const afterCodeHash = sourceTx?.account_state_after?.code_hash ?? fallbackCodeHash
    const contractCodeHash = beforeCodeHash ?? afterCodeHash
    tx.contractAbi = contractCodeHash ? (abiByCodeHash.get(contractCodeHash) ?? undefined) : undefined
    tx.parsedStorageBefore = decodeStorageDataCell(
      sourceTx?.account_state_before?.data_boc,
      beforeCodeHash ? abiByCodeHash.get(beforeCodeHash) : undefined,
    )
    tx.parsedStorageAfter = decodeStorageDataCell(
      sourceTx?.account_state_after?.data_boc,
      afterCodeHash ? abiByCodeHash.get(afterCodeHash) : undefined,
    )
  }

  const contracts = new Map<string, ContractData>()
  await Promise.all(
    traceAddressOrder.map(async (addr, index) => {
      const letter = String.fromCodePoint(65 + index)
      const displayAddr = normalizeAddress(addr, addressFormat)
      const customName = await fetchName(addr)
      const abi = abiByCodeHash.get(addressToCodeHash.get(addressKey(addr)) ?? "")
      contracts.set(addr, {
        displayName: customName || formatDisplayAddress(displayAddr, true, addressFormat),
        address: Address.parse(addr),
        letter,
        abi,
      })
    }),
  )

  return {
    transactions: processed,
    contracts,
    compilerAbisByCodeHash: new Map(abiByCodeHash),
    verifiedSourcesByCodeHash,
    valueFlow: buildValueFlowItems(processed),
  }
}

async function loadVerifiedSourcesByCodeHash({
  metadataRegistry,
  codeHashes,
  shouldContinue,
}: {
  readonly metadataRegistry: ExplorerMetadataRegistry
  readonly codeHashes: readonly string[]
  readonly shouldContinue: () => boolean
}): Promise<Map<string, ContractVerifiedSource> | undefined> {
  const uniqueCodeHashes = [...new Set(codeHashes.filter(codeHash => codeHash.trim().length > 0))]
  if (uniqueCodeHashes.length === 0) {
    return new Map()
  }

  const sources = await Promise.all(
    uniqueCodeHashes.map(
      async (codeHash): Promise<readonly [string, ContractVerifiedSource] | undefined> => {
        try {
          const source = await metadataRegistry.getSource({codeHash})
          if (!source.verified || source.bundles.length === 0) {
            return undefined
          }
          return [codeHash, source] as const
        } catch (error) {
          console.debug(`Failed to fetch verified source for ${codeHash}`, error)
          return undefined
        }
      },
    ),
  )

  if (!shouldContinue()) {
    return undefined
  }

  return new Map(
    sources.filter(
      (entry): entry is readonly [string, ContractVerifiedSource] => entry !== undefined,
    ),
  )
}

function collectTraceAddressOrder(processed: readonly TransactionInfo[]): readonly string[] {
  const addresses = new Set<string>()

  const visit = (tx: TransactionInfo) => {
    const address = tx.address?.toString()
    if (address) {
      addresses.add(address)
    }

    for (const child of [...tx.children].sort(compareTransactionInfoByLt)) {
      visit(child)
    }
  }

  for (const tx of [...processed].filter(tx => !tx.parent).sort(compareTransactionInfoByLt)) {
    visit(tx)
  }

  return [...addresses]
}

function compareTransactionInfoByLt(left: TransactionInfo, right: TransactionInfo): number {
  const leftLt = parseBigInt(left.lt)
  const rightLt = parseBigInt(right.lt)
  if (leftLt === rightLt) {
    return 0
  }
  return leftLt < rightLt ? -1 : 1
}

function parseBigInt(value: string | undefined): bigint {
  try {
    return value === undefined ? 0n : BigInt(value)
  } catch {
    return 0n
  }
}
